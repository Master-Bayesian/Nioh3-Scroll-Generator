"""Application version and embedded release-channel configuration."""

APP_VERSION = "0.6.6"
APP_ID = "nioh3-scroll-generator"
APP_AUTHORS = ("MasterBayesian", "Saber_Li")
CONTACT_QQ_GROUP = "1106302479"
PROJECT_GITHUB_URL = "https://github.com/Master-Bayesian/Nioh3-Scroll-Generator"

# The release URL and verification key are compiled into the executable so end
# users never manage a separate updater configuration file. The signing private
# key exists only as a GitHub Actions secret.
UPDATE_STABLE_MANIFEST_URL = (
    "https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/"
    "releases/latest/download/latest.json"
)
UPDATE_RELEASES_API_URL = (
    "https://api.github.com/repos/Master-Bayesian/"
    "Nioh3-Scroll-Generator/releases?per_page=20"
)
# Backward-compatible name used by older integrations and tests.
UPDATE_MANIFEST_URL = UPDATE_STABLE_MANIFEST_URL
UPDATE_PUBLIC_KEY_BASE64 = "c6oPCnJE4B+7ZnDUkZRJzUo3PZQmlM/eMlFqRC1h3dU="
