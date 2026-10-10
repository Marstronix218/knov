// Point every download button at the newest published DMG.
// Releases are published as prereleases, which GitHub's /releases/latest
// skips, so read the release list and take the first one with a .dmg.
// Without network access or a release, the buttons keep their fallback
// link to the Releases page.
const REPO = "Marstronix218/knov";

async function findLatestDmg() {
  const response = await fetch(`https://api.github.com/repos/${REPO}/releases?per_page=10`, {
    headers: { Accept: "application/vnd.github+json" },
  });
  if (!response.ok) return null;
  for (const release of await response.json()) {
    if (release.draft) continue;
    const dmg = release.assets.find((asset) => asset.name.endsWith(".dmg"));
    if (dmg) return { url: dmg.browser_download_url, version: release.tag_name, size: dmg.size };
  }
  return null;
}

function formatSize(bytes) {
  return `${Math.round(bytes / 1024 / 1024)} MB`;
}

findLatestDmg()
  .then((dmg) => {
    if (!dmg) return;
    document.querySelectorAll("[data-download]").forEach((link) => { link.href = dmg.url; });
    document.querySelectorAll("[data-release-meta]").forEach((meta) => {
      meta.textContent = `${dmg.version} · ${formatSize(dmg.size)} · macOS 13 or later · Apple Silicon and Intel`;
    });
  })
  .catch(() => {});

document.querySelectorAll("[data-copy]").forEach((button) => {
  button.addEventListener("click", async () => {
    const text = document.getElementById(button.dataset.copy).textContent;
    try {
      await navigator.clipboard.writeText(text);
      button.textContent = "Copied";
    } catch {
      const range = document.createRange();
      range.selectNodeContents(document.getElementById(button.dataset.copy));
      getSelection().removeAllRanges();
      getSelection().addRange(range);
      button.textContent = "Selected";
    }
    setTimeout(() => { button.textContent = "Copy"; }, 1600);
  });
});
