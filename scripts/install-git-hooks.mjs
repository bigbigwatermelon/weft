import { existsSync } from 'node:fs'
import { spawnSync } from 'node:child_process'

if (process.env.CI === 'true' || process.env.NODE_ENV === 'production') {
  process.exit(0)
}

if (!existsSync('.git') || !existsSync('.githooks')) {
  process.exit(0)
}

const result = spawnSync('git', ['config', 'core.hooksPath', '.githooks'], {
  stdio: 'inherit',
})

if (result.error) {
  console.warn(`Could not install git hooks: ${result.error.message}`)
  process.exit(0)
}

process.exit(result.status ?? 0)
