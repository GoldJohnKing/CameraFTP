#!/usr/bin/env node
import { readdirSync, readFileSync, statSync } from 'fs';
import { join } from 'path';

const LEGACY = ['getThumbnail', 'cleanupThumbnailsNotInList', 'listGalleryMedia'];
const EXCLUDE = ['node_modules', '.git', 'dist', 'build', 'target', 'gen', '.worktrees', '.venv', '.cursor', '.ruff_cache', 'out', 'logs', 'tmp', 'GalleryBridge.kt'];
const EXTENSIONS = ['.ts', '.tsx'];

function walk(dir) {
  let found = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (EXCLUDE.some(e => full.includes(e))) continue;
    const st = statSync(full);
    if (st.isDirectory()) { found = found.concat(walk(full)); }
    else if (EXTENSIONS.some(e => full.endsWith(e))) {
      const content = readFileSync(full, 'utf8');
      for (const pat of LEGACY) {
        if (content.includes(pat)) found.push(`${full}: ${pat}`);
      }
    }
  }
  return found;
}

const hits = walk('.');
if (hits.length > 0) {
  console.error('Legacy thumbnail patterns found:');
  hits.forEach(h => console.error(`  ${h}`));
  process.exit(1);
}
console.log('No legacy thumbnail patterns found.');
