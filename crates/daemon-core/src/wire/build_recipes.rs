//! Bounded recipe capabilities; absence in an old claim means desktop release only.
use serde::{Deserialize, Serialize};

pub const SOURCE_IMAGE_PRIVATE_CHECKOUT: &str = "source_image_private_checkout_v1";
pub const REPOSITORY_TOKEN: &str = "repository_token";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<String>", into = "Vec<String>")]
pub struct BuildRecipes(Vec<String>);

impl TryFrom<Vec<String>> for BuildRecipes {
    type Error = &'static str;

    fn try_from(recipes: Vec<String>) -> Result<Self, Self::Error> {
        if recipes.len() > 8
            || recipes.iter().any(|recipe| {
                recipe.is_empty()
                    || recipe.len() > 64
                    || !recipe.bytes().all(|byte| {
                        byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'
                    })
            })
        {
            return Err("Invalid build recipe capabilities.");
        }
        let mut unique = recipes.clone();
        unique.sort();
        unique.dedup();
        if unique.len() != recipes.len() {
            return Err("Duplicate build recipe capability.");
        }
        Ok(Self(recipes))
    }
}

impl From<BuildRecipes> for Vec<String> {
    fn from(recipes: BuildRecipes) -> Self {
        recipes.0
    }
}

impl BuildRecipes {
    #[must_use]
    pub fn supports(&self, recipe: &str) -> bool {
        self.0.iter().any(|supported| supported == recipe)
    }
}
