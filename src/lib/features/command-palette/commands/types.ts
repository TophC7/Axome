import type { IconData } from '$lib/icons/Icon.svelte'

export interface Command {
  id: string
  label: string
  subtitle?: string
  icon?: IconData
  iconSrc?: string
  /** Hugeicons kebab slug from user config (e.g. tasks), rendered lazily via NamedIcon; unknown slugs render TerminalIcon. */
  iconName?: string
  keywords: string[]
  defaultKeybindings?: string[]
  onSelect: () => void
}

export interface CommandGroup {
  heading: string
  commands: Command[]
}
