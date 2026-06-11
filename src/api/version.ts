import { invokeCommand } from './index'

export async function getAppVersion(): Promise<string> {
  return invokeCommand<string>('get_app_version', {})
}

interface GitHubTag {
  name: string
}

/**
 * Fetch the latest tag from the GitHub repo.
 * When a `github_token` is stored in config it is sent as a Bearer token
 * to avoid the 60-req/hour anonymous rate-limit.
 */
export async function fetchLatestGitHubTag(
  token?: string | null,
): Promise<string | null> {
  try {
    const headers: Record<string, string> = {
      Accept: 'application/vnd.github+json',
    }
    if (token) {
      headers.Authorization = `Bearer ${token}`
    }
    const response = await fetch(
      'https://api.github.com/repos/zhangsubo/skillsPanel/tags?per_page=1',
      { headers },
    )
    if (!response.ok) return null
    const tags: GitHubTag[] = await response.json()
    if (tags.length === 0) return null
    return tags[0].name
  } catch {
    return null
  }
}

function parseVersion(v: string): number[] {
  return v
    .replace(/^v/, '')
    .split('.')
    .map((n) => parseInt(n, 10))
    .filter((n) => !isNaN(n))
}

export function isNewerVersion(current: string, latest: string): boolean {
  const c = parseVersion(current)
  const l = parseVersion(latest)
  for (let i = 0; i < Math.max(c.length, l.length); i++) {
    const cv = c[i] || 0
    const lv = l[i] || 0
    if (lv > cv) return true
    if (lv < cv) return false
  }
  return false
}

export async function checkForUpdate(): Promise<{
  hasUpdate: boolean
  currentVersion: string
  latestVersion: string | null
}> {
  const currentVersion = await getAppVersion()
  // Read the optional GitHub PAT so the tags request isn't rate-limited.
  let token: string | null = null
  try {
    token = await invokeCommand<string | null>('get_config_value', {
      key: 'github_token',
    })
  } catch {
    // No token stored — proceed anonymously.
  }
  const latestVersion = await fetchLatestGitHubTag(token)
  const hasUpdate = latestVersion
    ? isNewerVersion(currentVersion, latestVersion)
    : false
  return { hasUpdate, currentVersion, latestVersion }
}
