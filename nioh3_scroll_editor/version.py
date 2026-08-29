"""Application version and embedded release-channel configuration."""

APP_VERSION = "0.5.4"
APP_ID = "nioh3-scroll-generator"
APP_AUTHORS = ("MasterBayesian", "Saber_Li")
CONTACT_QQ_GROUP = "1106302479"
PROJECT_GITHUB_URL = "https://github.com/Master-Bayesian/Nioh3-Scroll-Generator"

# These values are intentionally fail-closed until the official public release
# repository and signing key are created. They are compiled into the executable
# so end users never manage a separate configuration file.
UPDATE_MANIFEST_URL = (
    "https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/"
    "releases/latest/download/latest.json"
)
UPDATE_PUBLIC_KEY_BASE64 = ""
