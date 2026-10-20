'use strict';
function buildReport(config) {
  if (config.status !== 'ready') throw new Error('Report is not ready');
  return { title: 'Synthetic report', count: config.items.length };
}
module.exports = { buildReport };
