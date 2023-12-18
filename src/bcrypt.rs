const DEFAULT_PASSWORD: &str = "fakepassword";
const DEFAULT_COST: u32 = bcrypt::DEFAULT_COST;

use derive_more::{Display, From, Into};

#[derive(Debug, Into, From, Display)]
pub struct BcryptHash(pub String);

pub fn get_default_hash(cost: Option<u32>) -> Option<BcryptHash> {
    bcrypt::hash(DEFAULT_PASSWORD, cost.unwrap_or(DEFAULT_COST))
        .ok()
        .map(|v| v.into())
}

pub fn get_cost(hash: &String) -> Option<u32> {
    let parts: Vec<&str> = hash.split('$').collect();
    parts
        .get(2)
        .map(|v| v.parse::<u32>().ok())
        .flatten()
        .filter(|v| *v >= 4u32)
}

#[cfg(test)]
mod tests {
    mod bcrypt {
        use crate::bcrypt::get_cost;

        #[test]
        fn none_if_empty_string() {
            assert_eq!(None, get_cost(&"".to_string()))
        }

        #[test]
        fn none_if_unparseable_string() {
            assert_eq!(None, get_cost(&"$12".to_string()))
        }

        #[test]
        fn none_if_incomplete_string() {
            assert_eq!(None, get_cost(&"$2a$".to_string()))
        }

        #[test]
        fn cost_128() {
            assert_eq!(
                Some(255u32),
                get_cost(
                    &"$2a$255$R9h/cIPz0gi.URNNX3kh2OPST9/PgBkqquzi.Ss7KIUgO2t0jWMUW".to_string()
                )
            )
        }

        #[test]
        fn cost_10() {
            assert_eq!(
                Some(10u32),
                get_cost(
                    &"$2a$10$R9h/cIPz0gi.URNNX3kh2OPST9/PgBkqquzi.Ss7KIUgO2t0jWMUW".to_string()
                )
            )
        }

        #[test]
        fn cost_less_than_4() {
            assert_eq!(
                None,
                get_cost(
                    &"$2a$3$R9h/cIPz0gi.URNNX3kh2OPST9/PgBkqquzi.Ss7KIUgO2t0jWMUW".to_string()
                )
            )
        }
    }
}
