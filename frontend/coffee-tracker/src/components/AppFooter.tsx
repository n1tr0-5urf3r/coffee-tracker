import { useVersion } from '../api/queries'
import { trackerVersion, frontendVersion } from '../version'
import { branding } from '../config/branding'

export function footerVersionLine(backendVersion: string) {
  return `${branding.appName} v${trackerVersion} · Frontend v${frontendVersion} · Backend v${backendVersion}`
}

export function AppFooter() {
  const version = useVersion()
  const backendVersion = version.data?.backend_version ?? 'unknown'

  return (
    <footer className="mt-8 border-t border-berry-100 px-1 py-6 text-center text-xs font-bold leading-relaxed text-berry-700 dark:border-white/10 dark:text-rose-100">
      <p>{footerVersionLine(backendVersion)}</p>
      <p className="mt-1">
        Made with love and questionable vibe coding by Fabian, Codex, and Claude.
      </p>
      <a
        href={branding.githubUrl}
        target="_blank"
        rel="noreferrer"
        className="mt-2 inline-flex items-center gap-1.5 rounded-full px-3 py-1 transition hover:bg-primary/10 hover:text-primary dark:hover:bg-white/10 dark:hover:text-blush"
      >
        <GitHubIcon />
        View on GitHub
      </a>
    </footer>
  )
}

function GitHubIcon() {
  return (
    <svg viewBox="0 0 24 24" className="size-4" fill="currentColor" aria-hidden="true">
      <path d="M12 .7a11.5 11.5 0 0 0-3.6 22.4c.6.1.8-.2.8-.5v-2.2c-3.3.7-4-1.4-4-1.4-.5-1.4-1.3-1.8-1.3-1.8-1.1-.7.1-.7.1-.7 1.2.1 1.8 1.2 1.8 1.2 1.1 1.8 2.8 1.3 3.5 1 .1-.8.4-1.3.8-1.6-2.7-.3-5.5-1.3-5.5-5.7 0-1.3.5-2.3 1.2-3.1-.1-.3-.5-1.6.1-3.1 0 0 1-.3 3.2 1.2a11 11 0 0 1 5.8 0c2.2-1.5 3.2-1.2 3.2-1.2.6 1.5.2 2.8.1 3.1.8.8 1.2 1.8 1.2 3.1 0 4.4-2.8 5.4-5.5 5.7.4.4.8 1.1.8 2.2v3.3c0 .3.2.6.8.5A11.5 11.5 0 0 0 12 .7Z" />
    </svg>
  )
}
