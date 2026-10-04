const crypto = require('node:crypto');

function sha256Json(value) {
  return `0x${crypto.createHash('sha256').update(JSON.stringify(value)).digest('hex')}`;
}

function normalizedSha(ch) {
  return `0x${ch.repeat(64)}`;
}

const genesisHash = normalizedSha('0');

function categoryForPath(path) {
  const category = path.replace(/^\/+/, '').split('/')[0] || '';
  return ['writing', 'projects', 'papers', 'talks'].includes(category) ? category : 'misc';
}

function makeLedgerEntry({ route, path, files, date = null }) {
  const content_sha256 = sha256Json(files);
  const entry = {
    id: `route:${route}`,
    route,
    path,
    category: categoryForPath(path),
    content_files: files,
    content_sha256
  };
  return {
    sort_key: { date, path },
    entry
  };
}

function makeLedger(inputs) {
  const blocks = [...inputs].sort((left, right) => {
    const leftDate = left.sort_key.date;
    const rightDate = right.sort_key.date;
    if (leftDate === null && rightDate !== null) return -1;
    if (leftDate !== null && rightDate === null) return 1;
    if (leftDate !== rightDate) return leftDate < rightDate ? -1 : 1;
    if (left.sort_key.path !== right.sort_key.path) {
      return left.sort_key.path < right.sort_key.path ? -1 : 1;
    }
    return 0;
  }).map((input, index) => ({
    height: index + 1,
    sort_key: input.sort_key,
    prev_block_sha256: index === 0 ? genesisHash : '',
    block_sha256: '',
    entry: input.entry
  }));

  for (let index = 0; index < blocks.length; index += 1) {
    if (index > 0) {
      blocks[index].prev_block_sha256 = blocks[index - 1].block_sha256;
    }
    const block = blocks[index];
    block.block_sha256 = sha256Json({
      height: block.height,
      sort_key: block.sort_key,
      prev_block_sha256: block.prev_block_sha256,
      entry: block.entry
    });
  }

  return {
    version: 1,
    scheme: 'websh.content-ledger.v1',
    hash: 'sha256',
    genesis_hash: genesisHash,
    blocks,
    block_count: blocks.length,
    chain_head: blocks.length === 0 ? genesisHash : blocks[blocks.length - 1].block_sha256
  };
}

function emptyLedger() { return makeLedger([]); }

module.exports = { makeLedger, makeLedgerEntry, normalizedSha, emptyLedger };
