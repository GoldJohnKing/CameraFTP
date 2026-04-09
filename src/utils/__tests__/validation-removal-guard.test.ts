import * as fs from 'fs';
import * as path from 'path';
import { expect, test } from 'vitest';

test('no source file imports from utils/validation', () => {
  const srcRoot = path.resolve(__dirname, '..');
  const files = walkTsFiles(srcRoot);
  const violators: string[] = [];

  for (const file of files) {
    if (file.includes('validation-removal-guard')) continue;
    if (file.includes('node_modules')) continue;
    const content = fs.readFileSync(file, 'utf-8');
    if (
      content.includes("from '../utils/validation'") ||
      content.includes("from '../../utils/validation'") ||
      content.includes("from './validation'")
    ) {
      violators.push(path.relative(srcRoot, file));
    }
  }

  expect(violators).toEqual([]);
});

function walkTsFiles(dir: string): string[] {
  const results: string[] = [];
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory() && entry.name !== 'node_modules') {
      results.push(...walkTsFiles(full));
    } else if (entry.isFile() && /\.(ts|tsx)$/.test(entry.name)) {
      results.push(full);
    }
  }
  return results;
}
