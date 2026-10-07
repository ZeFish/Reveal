/**
 * What went wrong with an AI request, in words a person can act on.
 *
 * The provider's own error reaches the panel as the HTTP library worded it —
 * `http: https://api.anthropic.com/v1/messages: status code 401` — which says nothing about
 * what to do. The common cases have one answer each, and it is nearly always in Settings.
 *
 * @param {unknown} error
 * @returns {string}
 */
export function explainAiError(error) {
  const text = String(error ?? "");
  if (/no vision api key/i.test(text)) {
    return "No AI key yet — add one in Settings → AI & Automation.";
  }
  if (/\b(401|403)\b/.test(text)) {
    return "The AI provider refused the key (error 401). Check it in Settings → AI & Automation.";
  }
  if (/\b429\b/.test(text)) {
    return "The AI provider is rate-limiting this key (error 429). Try again in a minute.";
  }
  if (/\b5\d\d\b/.test(text)) {
    return "The AI provider had a problem on its side. Try again in a moment.";
  }
  if (/timed? ?out|timeout|dns|connect|network/i.test(text)) {
    return "Could not reach the AI provider. Check the connection and try again.";
  }
  return text;
}
