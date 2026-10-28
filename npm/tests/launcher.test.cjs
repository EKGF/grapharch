const assert = require('node:assert/strict');
const {mkdtempSync, mkdirSync, writeFileSync, chmodSync, rmSync} = require('node:fs');
const {tmpdir} = require('node:os');
const {join} = require('node:path');
const {spawnSync} = require('node:child_process');
const {test} = require('node:test');
const {platformPackage, resolveBinary} = require('../lib/launcher.cjs');

test('selects only a declared operating system, architecture, and libc', () => {
  assert.equal(platformPackage({platform:'linux', arch:'x64', glibc:'2.28'}), '@grapharch/cli-linux-x64');
  assert.equal(platformPackage({platform:'darwin', arch:'arm64'}), '@grapharch/cli-darwin-arm64');
  assert.equal(platformPackage({platform:'win32', arch:'x64'}), '@grapharch/cli-win32-x64');
  assert.throws(() => platformPackage({platform:'linux', arch:'x64'}), /glibc/);
  assert.throws(() => platformPackage({platform:'linux', arch:'x64', glibc:'2.17'}), /2.28/);
  assert.throws(() => platformPackage({platform:'freebsd', arch:'x64'}), /unsupported/i);
  assert.throws(() => platformPackage({platform:'linux', arch:'riscv64', glibc:'2.40'}), /unsupported/i);
});

test('uses only the exact package/version binary; never a PATH fallback', () => {
  const root=mkdtempSync(join(tmpdir(),'grapharch-launcher-'));
  try {
    mkdirSync(join(root,'bin'));
    writeFileSync(join(root,'package.json'),JSON.stringify({name:'@grapharch/cli-darwin-arm64',version:require('../package.json').version}));
    writeFileSync(join(root,'bin/grapharch'),'fixture');chmodSync(join(root,'bin/grapharch'),0o755);
    const options={platform:'darwin',arch:'arm64',resolve:()=>join(root,'package.json')};
    assert.equal(resolveBinary(options),join(root,'bin/grapharch'));
    writeFileSync(join(root,'package.json'),JSON.stringify({name:'@grapharch/cli-darwin-arm64',version:'9.9.9'}));
    assert.throws(()=>resolveBinary(options),/version/);
    assert.throws(()=>resolveBinary({...options,resolve:()=>{throw Error('missing')}}),/platform package/);
  } finally {rmSync(root,{recursive:true,force:true});}
});

test('missing installed platform package terminates with an actionable error', () => {
  const result=spawnSync(process.execPath,[join(__dirname,'../bin/grapharch.cjs'),'--version'],{encoding:'utf8'});
  assert.equal(result.status,1);
  assert.match(result.stderr,/platform package/);
  assert.doesNotMatch(result.stderr,/cargo|fallback|downloading/i);
});

test('launcher preserves arguments, working directory, and native exit status', () => {
  const {copyFileSync,symlinkSync,realpathSync}=require('node:fs');
  const root=mkdtempSync(join(tmpdir(),'grapharch wrapper with spaces '));
  try {
    const pkg=join(root,'package');
    mkdirSync(join(pkg,'bin'),{recursive:true});mkdirSync(join(pkg,'lib'));
    for(const file of ['package.json','bin/grapharch.cjs','lib/launcher.cjs']) copyFileSync(join(__dirname,'..',file),join(pkg,file));
    const name=platformPackage({platform:process.platform,arch:process.arch,glibc:process.report.getReport().header.glibcVersionRuntime});
    const native=join(pkg,'node_modules',name);
    mkdirSync(join(native,'bin'),{recursive:true});
    writeFileSync(join(native,'package.json'),JSON.stringify({name,version:require('../package.json').version}));
    const binary=join(native,'bin',process.platform==='win32'?'grapharch.exe':'grapharch');
    symlinkSync(process.execPath,binary,'file');
    const program='process.stdout.write(JSON.stringify({args:process.argv.slice(1),cwd:process.cwd()}));process.exit(37)';
    const result=spawnSync(process.execPath,[join(pkg,'bin/grapharch.cjs'),'--eval',program,'--','argument with spaces','--flag'],{cwd:root,encoding:'utf8'});
    assert.equal(result.status,37,result.stderr);
    assert.deepEqual(JSON.parse(result.stdout),{args:['argument with spaces','--flag'],cwd:realpathSync(root)});
    if (process.platform !== 'win32') {
      const signalled=spawnSync(process.execPath,[join(pkg,'bin/grapharch.cjs'),'--eval','process.kill(process.pid,"SIGTERM")'],{encoding:'utf8'});
      assert.equal(signalled.signal,'SIGTERM','native signal termination must not become a successful exit');
    }
  } finally {rmSync(root,{recursive:true,force:true});}
});
