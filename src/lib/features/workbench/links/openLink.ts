import { backend } from '$lib/api/backend'
import { notify } from '$lib/features/notifications/state.svelte'
import { getErrorMessage } from '$lib/utils/client-error'
import { basename, dirname, isPathWithin, splitRemotePath, toProjectRelativePath } from '$lib/utils/paths'
import { revealPath } from '$lib/features/files/explorer.svelte'
import { openFolder } from '$lib/features/workbench/state.svelte'
import { openTextFile, type TextRevealTarget } from '$lib/features/workbench/surfaces/text/service.svelte'
import { platform } from '$lib/platform'
import { parseFilePathAndLocation, parseLinkTarget, type ParsedLinkTarget } from './parseLink'
const GITHUB_REMOTE_REGEX = /(?:github\.com[:/])([^/\s]+)\/([^/\s]+?)(?:\.git)?(?:\s|$)/
const MAX_GIT_CACHE_ENTRIES = 16
const gitRepoCache = new Map<string, { owner: string; repo: string } | null>()

const OMP_SCHEMES: Record<string, true> = {
  omp: true,
  skill: true,
  rule: true,
  local: true,
  artifact: true,
  history: true,
  agent: true
}
async function getGitHubRepo(folderPath: string): Promise<{ owner: string; repo: string } | null> {
  if (gitRepoCache.has(folderPath)) {
    return gitRepoCache.get(folderPath) ?? null
  }

  try {
    const { content } = await backend.files.read(folderPath, '.git/config')
    const match = content.match(GITHUB_REMOTE_REGEX)
    const result = match ? { owner: match[1], repo: match[2] } : null
    if (gitRepoCache.size >= MAX_GIT_CACHE_ENTRIES) gitRepoCache.clear()
    gitRepoCache.set(folderPath, result)
    return result
  } catch {
    // Not a git repo or unable to read .git/config
    if (gitRepoCache.size >= MAX_GIT_CACHE_ENTRIES) gitRepoCache.clear()
    gitRepoCache.set(folderPath, null)
    return null
  }
}

/**
 * Open a link from terminal output or markdown view.
 *
 * @param rawTarget The clicked link URL, URI, or file path.
 * @param folderPath Active workbench folder path.
 */
export async function openLink(rawTarget: string, folderPath?: string | null): Promise<boolean> {
  try {
    const target = parseLinkTarget(rawTarget)
    if (target.kind === 'anchor') {
      // Fragment-only link (#section); handled within document or no-op
      return true
    }

    if (target.kind === 'web' && target.url) {
      await platform.links.openExternal(target.url)
      return true
    }

    if (target.kind === 'uri') {
      return await handleUriLink(target, folderPath)
    }

    if (target.kind === 'file') {
      return await openFileLocation(target.path ?? rawTarget, target.reveal, folderPath)
    }
  } catch (error) {
    notify.error('Failed to open link', getErrorMessage(error))
    return false
  }

  return false
}

async function handleUriLink(target: ParsedLinkTarget, folderPath?: string | null): Promise<boolean> {
  const scheme = target.scheme
  const fullUrl = target.url ?? target.raw

  if (scheme === 'file') {
    return handleFileScheme(fullUrl, folderPath)
  }

  if (scheme === 'sworm') {
    await openFolder(fullUrl)
    return true
  }

  if (scheme === 'issue' || scheme === 'pr') {
    return handleIssueOrPrScheme(scheme, fullUrl, folderPath)
  }

  if (scheme && OMP_SCHEMES[scheme]) {
    return handleOmpScheme(fullUrl, target.reveal, folderPath)
  }

  // Fallback: try opening with system opener
  try {
    await platform.links.openExternal(fullUrl)
    return true
  } catch {
    notify.error('Unsupported link', `Scheme "${scheme}://" is not supported.`)
    return false
  }
}

async function handleOmpScheme(
  fullUrl: string,
  reveal?: TextRevealTarget | null,
  folderPath?: string | null
): Promise<boolean> {
  try {
    const resolved = await backend.sessions.ompResolveUri(fullUrl, folderPath)
    if (resolved.is_dir) {
      await openFolder(resolved.path)
      return true
    }
    const dir = dirname(resolved.path)
    const file = basename(resolved.path)
    if (dir && file) {
      await openTextFile(dir, file, { temporary: false, reveal })
      return true
    }
  } catch (err) {
    notify.error('Cannot open link', getErrorMessage(err))
    return false
  }
  return false
}

async function handleFileScheme(fullUrl: string, folderPath?: string | null): Promise<boolean> {
  let parsedUrl: URL
  try {
    parsedUrl = new URL(fullUrl)
  } catch {
    const rawPath = fullUrl.replace(/^file:\/\//, '')
    return openFileLocation(rawPath, null, folderPath)
  }

  let absPath = decodeURIComponent(parsedUrl.pathname)
  let reveal: TextRevealTarget | null = null
  if (parsedUrl.hash) {
    const { reveal: hashReveal } = parseFilePathAndLocation(`dummy${parsedUrl.hash}`)
    reveal = hashReveal
  }

  if (!reveal) {
    const parsed = parseFilePathAndLocation(absPath)
    absPath = parsed.filePath
    reveal = parsed.reveal
  }

  return openFileLocation(absPath, reveal, folderPath)
}

async function handleIssueOrPrScheme(
  scheme: 'issue' | 'pr',
  fullUrl: string,
  folderPath?: string | null
): Promise<boolean> {
  const match = fullUrl.match(/^(?:issue|pr):\/\/(?:([^/]+)\/([^/]+)\/)?(\d+)/)
  if (!match) {
    notify.error('Invalid link', `Cannot parse ${fullUrl}`)
    return false
  }

  let owner = match[1]
  let repo = match[2]
  const id = match[3]
  const section = scheme === 'issue' ? 'issues' : 'pull'

  if (!owner || !repo) {
    if (folderPath) {
      const git = await getGitHubRepo(folderPath)
      if (git) {
        owner = git.owner
        repo = git.repo
      }
    }
  }

  if (!owner || !repo) {
    notify.error('Cannot open link', `No repository identified for ${scheme} #${id}`)
    return false
  }

  const githubUrl = `https://github.com/${owner}/${repo}/${section}/${id}`
  await platform.links.openExternal(githubUrl)
  return true
}

/** Resolve a literal filesystem path on the active folder's host, without URL or location parsing. */
export async function resolveFileLocationPath(filePath: string, folderPath?: string | null): Promise<string> {
  if (filePath.startsWith('sworm://') && splitRemotePath(filePath)) return filePath
  const server = folderPath ? splitRemotePath(folderPath)?.server : undefined
  if (filePath === '~' || filePath.startsWith('~/')) {
    const home = await backend.folders.home(server)
    return `${home}${filePath.slice(1)}`
  }
  if (filePath.startsWith('/')) return server ? `sworm://${server}${filePath}` : filePath
  if (!folderPath) throw new Error(`No active project folder to resolve relative path "${filePath}"`)
  // Leave .. and symlinks to the filesystem; lexical normalization changes their meaning.
  return `${folderPath.replace(/\/+$/, '')}/${filePath}`
}

/** Open a literal path; unlike openLink, # and : are always filename bytes here. */
export async function openFileLocation(
  filePath: string,
  reveal?: TextRevealTarget | null,
  folderPath?: string | null
): Promise<boolean> {
  try {
    const targetPath = await resolveFileLocationPath(filePath, folderPath)
    const directory = await backend.folders.resolve(targetPath).catch(() => null)
    if (directory) {
      if (folderPath && isPathWithin(folderPath, directory.path)) {
        const relative = toProjectRelativePath(folderPath, directory.path)
        if (relative) {
          await revealPath(folderPath, relative)
          return true
        }
      }
      await openFolder(directory.path)
      return true
    }

    // Canonicalize the parent, not the filename: preserve .. through symlink parents
    // and keep filenames containing URL fragments or line-like suffixes untouched.
    const slash = targetPath.lastIndexOf('/')
    const file = targetPath.slice(slash + 1)
    if (!file) throw new Error(`Cannot resolve file "${filePath}"`)
    const parent = await backend.folders.resolve(targetPath.slice(0, slash + 1))
    const stat = await backend.files.stat(parent.path, file)
    if (!stat.regular) throw new Error(`Not a regular file: "${filePath}"`)
    const absolutePath = `${parent.path.replace(/\/+$/, '')}/${file}`
    const relative = folderPath ? toProjectRelativePath(folderPath, absolutePath) : null
    await openTextFile(relative !== null && folderPath ? folderPath : parent.path, relative ?? file, {
      temporary: false,
      reveal
    })
    return true
  } catch (error) {
    notify.error('Cannot open file', getErrorMessage(error))
    return false
  }
}
