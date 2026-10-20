# Try Knov (early access)

Thanks for helping test Knov. It's a Mac app that keeps track of what you work on across apps and websites, so you can pick up where you left off and ask AI about your own work without explaining everything again.

This is an early test version. Some things will be rough, and your feedback decides what gets built next.

## What you need

- A Mac with macOS 13 (Ventura) or later
- About 5 minutes to set it up
- Optional: [Ollama](https://ollama.com/download) for free AI that runs on your Mac, or an OpenAI or Anthropic API key

## Install

1. Download the latest **Knov .dmg** from the [Releases page](https://github.com/Marstronix218/knov/releases/latest).
2. Open the `.dmg` and drag **Knov** into **Applications**.
3. Open Knov from Applications.

**If macOS says it "can't verify" Knov:** this test build isn't signed through the App Store process yet.

1. Click **Done** (not "Move to Trash").
2. Open **System Settings → Privacy & Security**.
3. Scroll down. Next to the message about Knov, click **Open Anyway**, then confirm.

You only need to do this once. If macOS instead says the app "is damaged," open Terminal and run the command below, then open Knov again:

```sh
xattr -dr com.apple.quarantine /Applications/Knov.app
```

## Setup (4 steps)

1. **Welcome.** Lists what Knov records and what it never records.
2. **Permissions.** Click **Allow** and accept the macOS prompt about *System Events*. This lets Knov see which app is in front. Then click **Open settings** and turn on Knov under **Accessibility** so it can read window titles. If the window-titles row still shows Off after you turn it on, quit Knov from the menu bar icon and open it again.
3. **Browser history (optional).** Choose Chrome, Arc, Brave, Edge, or Vivaldi profiles so Knov understands your projects from day one. Safari history isn't supported yet, but Safari page titles are still recorded through window titles.
4. **AI.** Choose one:
   - **On this Mac (recommended):** install [Ollama](https://ollama.com/download), open it, then run `ollama pull llama3.2` in Terminal. Knov finds it automatically. It's free, and nothing leaves your Mac.
   - **Cloud API key:** paste an OpenAI, Anthropic, or AWS Bedrock key. Answers are better, and usage is billed to your key.
   - **Skip:** threads and the activity timeline still work. You can connect AI later in **Settings**.

Knov keeps running in the **menu bar** when you close its window. Use the menu bar icon to pause or resume collection, or to quit Knov.

## What to try this week

- **Day 1:** Work as usual for an hour, then open **Now**. Does the thread at the top match what you were doing? Click **Resume thread**.
- **Day 2–3:** Open **Ask** and try "What have I been working on this week?" or "Draft a standup update from today's work."
- **Anytime:** Open **Memory** and correct anything Knov got wrong. Corrections always override what Knov guesses.
- **End of the week:** Click **Send feedback** in the sidebar. It takes two minutes.

Turn on **Settings → Labs** if you want to try experimental features: workflow detection, a work agent that drafts and opens things with your approval, and next-step predictions.

## Privacy

- Raw activity (apps, window titles, browser history) is stored only on your Mac and is deleted after 30 days.
- Knov never records keystrokes, screenshots, page or document contents, audio, or your clipboard.
- With a local AI, nothing leaves your Mac. With a cloud key, only a minimized summary and the questions you ask are sent, directly to the provider you chose.
- Feedback is never sent automatically. **Send feedback** opens a message you can review before you send it. Usage counts are optional and contain no titles, URLs, app names, or chat text.
- **Settings → Delete everything** erases all Knov data and stored keys.

## Uninstall

1. In Knov, open **Settings → Delete everything** (this also removes stored API keys).
2. Quit Knov from the menu bar icon.
3. Drag **Knov** from Applications to the Trash.

## Known limitations

- Safari and Firefox history import isn't supported yet.
- Small local models give simpler answers than cloud models. If answers feel weak, try a larger model such as `qwen3:8b`.
- Building your first profile on a local model can take a few minutes.
