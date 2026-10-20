/**
 * Build-time settings for tester builds. Set these as environment variables
 * (or GitHub repository variables for the release workflow) before building.
 */
const env = import.meta.env;

/** Address that receives tester feedback through the user's mail app. */
export const FEEDBACK_EMAIL: string = env.VITE_FEEDBACK_EMAIL?.trim() ?? "";
/** Optional feedback form (Tally, Google Forms, Typeform…); preferred over email when set. */
export const FEEDBACK_FORM_URL: string = env.VITE_FEEDBACK_URL?.trim() ?? "";
export const REPOSITORY_URL = "https://github.com/Marstronix218/knov";
export const RELEASES_URL: string = env.VITE_RELEASES_URL?.trim() || `${REPOSITORY_URL}/releases/latest`;
export const OLLAMA_DOWNLOAD_URL = "https://ollama.com/download";
export const SUGGESTED_LOCAL_MODEL = "llama3.2";
