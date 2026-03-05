#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Validator {
    pub index: u64,
    pub weight: u64,
    pub public_key: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatorSet {
    validators: Vec<Validator>,
    total_weight: u64,
}

impl ValidatorSet {
    pub fn new(validators: Vec<Validator>) -> Self {
        let total_weight = validators
            .iter()
            .fold(0u64, |acc, validator| acc.saturating_add(validator.weight));

        Self {
            validators,
            total_weight,
        }
    }

    pub fn get(&self, index: u64) -> Option<&Validator> {
        self.validators
            .iter()
            .find(|validator| validator.index == index)
    }

    pub fn total_weight(&self) -> u64 {
        self.total_weight
    }

    pub fn quorum_weight(&self) -> u64 {
        let f = self.total_weight.saturating_sub(1) / 3;
        (2 * f).saturating_add(1)
    }

    pub fn leader_for_round(&self, height: u64, round: u32) -> u64 {
        if self.validators.is_empty() {
            return 0;
        }

        let leader_position =
            (height.saturating_add(round as u64) % self.validators.len() as u64) as usize;
        self.validators[leader_position].index
    }

    pub fn len(&self) -> usize {
        self.validators.len()
    }

    pub fn is_empty(&self) -> bool {
        self.validators.is_empty()
    }

    pub fn weight_of(&self, index: u64) -> u64 {
        self.get(index).map_or(0, |validator| validator.weight)
    }
}

#[cfg(test)]
mod tests {
    use super::{Validator, ValidatorSet};

    fn validator(index: u64, weight: u64) -> Validator {
        Validator {
            index,
            weight,
            public_key: [index as u8; 32],
        }
    }

    #[test]
    fn computes_quorum_weight() {
        let set = ValidatorSet::new(vec![
            validator(0, 1),
            validator(1, 1),
            validator(2, 1),
            validator(3, 1),
        ]);

        assert_eq!(set.total_weight(), 4);
        assert_eq!(set.quorum_weight(), 3);
    }

    #[test]
    fn leader_selection_is_round_robin() {
        let set = ValidatorSet::new(vec![validator(10, 1), validator(20, 1), validator(30, 1)]);

        assert_eq!(set.leader_for_round(0, 0), 10);
        assert_eq!(set.leader_for_round(0, 1), 20);
        assert_eq!(set.leader_for_round(2, 0), 30);
        assert_eq!(set.leader_for_round(2, 2), 20);
    }
}
