use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::fmt;
use std::ops::Deref;

// Distinct type from a plain String (like a username) so passing the wrong one is a
// compile error instead of a runtime bug -- bet structs across casino/market/weather
// used to store the player as `pub player: String`, with nothing stopping a username
// from being passed where a UUID was expected. No validation, no behavior change --
// #[serde(transparent)] keeps the JSON wire format identical to a bare string.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PlayerUuid(pub String);

impl PlayerUuid {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// Lets `&PlayerUuid` deref-coerce to `&str` at call sites (e.g. passing straight into an
// ApiClient method that stays `&str`) without needing `.as_str()` everywhere -- one-directional,
// so nothing lets a raw String/username coerce back INTO a PlayerUuid.
impl Deref for PlayerUuid {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

// Lets a HashMap<PlayerUuid, V> be queried with a plain &str (e.g. `map.get(some_str)`)
// without constructing a PlayerUuid just to look one up -- same pattern as std's own
// `impl Borrow<str> for String`. Hash/Eq must agree with str's, which they do since
// PlayerUuid's derived Hash/PartialEq forward straight to the wrapped String's, and
// String's own Hash/Eq already forward to str's.
impl Borrow<str> for PlayerUuid {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PlayerUuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for PlayerUuid {
    fn from(s: String) -> Self {
        PlayerUuid(s)
    }
}

impl From<&str> for PlayerUuid {
    fn from(s: &str) -> Self {
        PlayerUuid(s.to_owned())
    }
}
