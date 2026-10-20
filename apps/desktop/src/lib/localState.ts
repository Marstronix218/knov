/**
 * Removes UI state Knov keeps in the browser storage of this window: the whole
 * session store (selected thread context, Ask transcript) and every `knov.*`
 * local preference, including the setup-complete flag. Other origins' keys are
 * left alone.
 */
export function clearLocalUiState(): void {
  try {
    sessionStorage.clear();
  } catch {
    // Storage can be unavailable; nothing to clear then.
  }
  try {
    const keys = Array.from({ length: localStorage.length }, (_, index) => localStorage.key(index))
      .filter((key): key is string => Boolean(key?.startsWith("knov.")));
    keys.forEach((key) => localStorage.removeItem(key));
  } catch {
    // Storage can be unavailable; nothing to clear then.
  }
}
