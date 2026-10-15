// Copies the built wiki to where the VM serves the game (npm run deploy): <workspace>/serve/wiki, or the directory
// given. The 8080 server serves <workspace>/serve/game, so a link serve/game/wiki -> ../wiki puts the wiki at
// http://<host>:8080/wiki/ next to the game without restarting it (a game deploy that empties serve/game drops the
// link; the next wiki deploy makes it again).
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const dist = path.join(root, '.vitepress', 'dist')
const target = path.resolve(process.argv[2] ?? path.join(root, '..', '..', 'serve', 'wiki'))
if (!fs.existsSync(path.join(dist, 'index.html'))) {
  console.error('no built wiki: run npm run build first')
  process.exit(1)
}
fs.rmSync(target, { recursive: true, force: true })
fs.cpSync(dist, target, { recursive: true })
console.log(`copied the wiki to ${target}`)
const game = path.join(path.dirname(target), 'game')
const link = path.join(game, 'wiki')
if (fs.existsSync(game) && !fs.existsSync(link)) {
  fs.symlinkSync(path.relative(game, target), link)
  console.log(`linked ${link} -> ${path.relative(game, target)}`)
}
