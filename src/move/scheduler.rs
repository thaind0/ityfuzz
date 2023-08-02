use std::collections::HashSet;
use libafl::corpus::{Corpus, Testcase};
use libafl::{Error, impl_serdeany};
use libafl::inputs::Input;
use libafl::prelude::{HasCorpus, HasMetadata, HasRand, Rand, Scheduler};
use move_vm_types::loaded_data::runtime_types::Type;
use revm_primitives::HashMap;
use serde::{Deserialize, Serialize};
use crate::r#move::input::{ConciseMoveInput, MoveFunctionInput, MoveFunctionInputT};
use crate::r#move::types::{MoveAddress, MoveFuzzState, MoveInfantStateState, MoveLoc, MoveStagedVMState};
use crate::r#move::vm_state::MoveVMState;
use crate::scheduler::{SortedDroppingScheduler, VoteData};
use crate::state::InfantStateState;

// A scheduler that ensures that all dependencies of a test case are available
// before executing it.

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MoveSchedulerMeta {
    // managed by MoveTestcaseScheduler
    pub current_idx: usize,
    pub current_deps: HashSet<Type>,
    pub testcase_to_deps: HashMap<usize, HashSet<Type>>,

    // managed by MoveVMStateScheduler
    pub deps_state_idx: HashMap<Type, HashSet<usize>>,
    pub state_idx_to_deps: HashMap<usize, HashSet<Type>>,
    pub unavailable_types: HashSet<Type>,
}

impl_serdeany!(MoveSchedulerMeta);


pub struct MoveTestcaseScheduler<SC> {
    pub inner: SC,
}


impl<SC> MoveTestcaseScheduler<SC> {
}


impl<SC> Scheduler<MoveFunctionInput, MoveFuzzState> for MoveTestcaseScheduler<SC>
    where SC: Scheduler<MoveFunctionInput, MoveFuzzState>
{
    fn on_add(&self, _state: &mut MoveFuzzState, _idx: usize) -> Result<(), Error> {
        let tc = _state.corpus().get(_idx).expect("Missing testcase");
        let input = tc.borrow().input().clone().expect("Missing input");
        let meta = _state.metadata_mut().get_mut::<MoveSchedulerMeta>().expect("Missing metadata");
        meta.testcase_to_deps.insert(_idx, input._deps.keys().cloned().collect::<HashSet<_>>());
        self.inner.on_add(_state, _idx)
    }

    fn next(&self, state: &mut MoveFuzzState) -> Result<usize, Error> {
        let next_idx = self.inner.next(state)?;
        let mut meta = state.metadata_mut().get_mut::<MoveSchedulerMeta>().expect("Missing metadata");
        meta.current_idx = next_idx;
        meta.current_deps = meta.testcase_to_deps.get(&next_idx).expect("Missing deps").clone();
        Ok(next_idx)
    }
}


pub struct MoveVMStateScheduler {
    pub inner: SortedDroppingScheduler<MoveStagedVMState, MoveInfantStateState>
}


impl Scheduler<MoveStagedVMState, MoveInfantStateState> for MoveVMStateScheduler {
    fn on_add(&self, state: &mut MoveInfantStateState, idx: usize) -> Result<(), Error> {
        let interesting_types = {
            let infant_state = state.corpus().get(idx).expect("Missing infant state")
                .borrow()
                .input()
                .clone()
                .expect("Missing input");
            infant_state.state.value_to_drop.keys().chain(
                infant_state.state.useful_value.keys()
            ).cloned().collect::<Vec<_>>()
        };
        let mut meta = state.metadata_mut().get_mut::<MoveSchedulerMeta>().expect("Missing metadata");
        interesting_types.iter().for_each(
            |v| {
                meta.deps_state_idx.entry(v.clone()).or_insert(Default::default()).insert(idx);
                meta.unavailable_types.remove(v);
            }
        );
        let entry = meta.state_idx_to_deps.entry(idx).or_insert(Default::default());
        interesting_types.iter().for_each(
            |v| {
                entry.insert(v.clone());
            }
        );
        self.inner.on_add(state, idx)
    }

    fn on_remove(&self, state: &mut MoveInfantStateState, idx: usize, _testcase: &Option<Testcase<MoveStagedVMState>>) -> Result<(), Error> {
        let mut meta = state.metadata_mut().get_mut::<MoveSchedulerMeta>().expect("Missing metadata");
        meta.state_idx_to_deps.get(&idx).expect("Missing state idx").iter().for_each(
            |v| {
                let all_idx = meta.deps_state_idx.get_mut(v).expect("Missing deps");
                all_idx.remove(&idx);
                if all_idx.is_empty() {
                    meta.unavailable_types.insert(v.clone());
                }
            }
        );
        meta.state_idx_to_deps.remove(&idx);
        self.inner.on_remove(state, idx, _testcase)
    }

    fn next(&self, state: &mut MoveInfantStateState) -> Result<usize, Error> {

        let mut sample_idx = vec![];
        {
            let mut meta = state.metadata_mut().get_mut::<MoveSchedulerMeta>().expect("Missing metadata");
            if meta.current_deps.len() == 0 {
                return self.inner.next(state);
            }
            for (idx, tys) in &meta.state_idx_to_deps {
                if tys.is_superset(&meta.current_deps) {
                    sample_idx.push(*idx);
                }
            }
        }

        let mut total_votes = 0;
        let mut sample_list = vec![];
        {
            let mut sampling_meta = state.metadata().get::<VoteData>().unwrap();
            for idx in sample_idx {
                let (votes, visits) = sampling_meta.votes_and_visits.get(&idx).unwrap();
                sample_list.push((idx, (*votes, *visits)));
                total_votes += *votes;
            }
        }


        let mut s: f64 = 0.0; // sum of votes so far
        let mut idx = usize::MAX;
        let threshold = (state.rand_mut().below(1000) as f64 / 1000.0)
            * total_votes as f64;

        for (sample_idx, (votes, _)) in &sample_list {
            s += *votes as f64;
            if s > threshold {
                idx = *sample_idx;
                break;
            }
        }

        if idx == usize::MAX {  // if we didn't find an input, just use the last one
            idx = sample_list.last().unwrap().0;
        }

        {
            let sampling_meta = state.metadata_mut().get_mut::<VoteData>().unwrap();
            sampling_meta.votes_and_visits.get_mut(&idx).unwrap().1 += 1;
            sampling_meta.visits_total += 1;
        }

        Ok(idx)
    }
}


