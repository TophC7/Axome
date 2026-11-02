import test, { describe } from 'node:test'
import assert from 'node:assert/strict'
import { resolveFileIcon } from './fileIconMap'

describe('fileIconMap', () => {
  test('resolves legacy .axome/tasks.json to plain json icon (not axome)', () => {
    assert.equal(resolveFileIcon('.axome/tasks.json'), 'json')
    assert.equal(resolveFileIcon('/repo/.axome/tasks.json'), 'json')
  })

  test('resolves .axome/tasks.jsonc and tasks.jsonc to axome icon', () => {
    assert.equal(resolveFileIcon('.axome/tasks.jsonc'), 'axome')
    assert.equal(resolveFileIcon('/repo/.axome/tasks.jsonc'), 'axome')
    assert.equal(resolveFileIcon('tasks.jsonc'), 'axome')
  })

  test('resolves .axome/settings.jsonc and settings.jsonc to axome icon', () => {
    assert.equal(resolveFileIcon('.axome/settings.jsonc'), 'axome')
    assert.equal(resolveFileIcon('/repo/.axome/settings.jsonc'), 'axome')
    assert.equal(resolveFileIcon('/home/user/.config/axome/settings.jsonc'), 'axome')
    assert.equal(resolveFileIcon('settings.jsonc'), 'axome')
  })

  test('preserves vscode icon for .vscode/tasks.json', () => {
    assert.equal(resolveFileIcon('.vscode/tasks.json'), 'vscode')
    assert.equal(resolveFileIcon('tasks.json'), 'vscode')
  })

  test('resolves generic jsonc files to json icon', () => {
    assert.equal(resolveFileIcon('other.jsonc'), 'json')
  })
})
