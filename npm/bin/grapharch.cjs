#!/usr/bin/env node
'use strict';
try { require('../lib/launcher.cjs').main(); }
catch (error) {
  console.error(`GraphArch: ${error.message}`);
  process.exit(1);
}
