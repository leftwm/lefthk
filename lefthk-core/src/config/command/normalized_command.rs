use serde::{Deserialize, Serialize};

type Content = String;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NormalizedCommand(pub Content);

impl TryFrom<String> for NormalizedCommand {
    type Error = ();

    fn try_from(value: String) -> Result<Self, Self::Error> {
        ron::from_str(&value).map_err(|err| {
            tracing::debug!("Couldn't load string: {}", err);
            ()
        })
    }
}
