// Sequences come from the `*.liga` glyph names in paper-design/paper-mono v1.000.
const LIGATURES = [
  '<-->',
  '<!--',
  '-->',
  '-:-',
  ':::',
  '::>',
  ':<:',
  ':>:',
  '!==',
  '..-',
  '...',
  '..<',
  '..=',
  '/==',
  '#![',
  '<--',
  '<->',
  '<::',
  '</>',
  '<<=',
  '<=>',
  '<|>',
  '<~>',
  '<~~',
  '=:=',
  '=!=',
  '=/=',
  '=<<',
  '===',
  '=>>',
  '>>=',
  '~~>',
  '->',
  '-~',
  '::',
  ':=',
  '!=',
  '!~',
  '.-',
  '.=',
  '[|',
  '{|',
  '/\\',
  '/=',
  '/>',
  '\\/',
  '&=',
  '#(',
  '#[',
  '#=',
  '<-',
  '</',
  '<<',
  '<=',
  '<>',
  '<|',
  '<~',
  '==',
  '=>',
  '=~',
  '>=',
  '>>',
  '|]',
  '|}',
  '|>',
  '~-',
  '~>',
  '~~'
] as const

// Longest-first so `===` wins over `==` at the same index.
const LIGATURE_RE = new RegExp(
  [...LIGATURES]
    .sort((a, b) => b.length - a.length)
    .map((s) => s.replace(/[.*+?^${}()|[\]\\/-]/g, '\\$&'))
    .join('|'),
  'g'
)

/** xterm character joiner: nonoverlapping UTF-16 ranges of ligature runs. */
export function joinLigatures(text: string): [number, number][] {
  return Array.from(text.matchAll(LIGATURE_RE), (m) => [m.index, m.index + m[0].length])
}
