import ts from 'typescript';
import {readFile,readdir} from 'node:fs/promises';
const root='apps/workshop',resources=JSON.parse(await readFile(`${root}/ui-locales.json`,'utf8'));
const missing=new Set();
for(const file of (await readdir(root)).filter(f=>/\.(tsx|ts)$/.test(f)&&!f.startsWith('verify'))){
 const source=ts.createSourceFile(file,await readFile(`${root}/${file}`,'utf8'),ts.ScriptTarget.Latest,true);
 function walk(node){
  if((ts.isStringLiteral(node)||ts.isJsxText(node)||ts.isNoSubstitutionTemplateLiteral(node)||ts.isTemplateHead(node)||ts.isTemplateMiddle(node)||ts.isTemplateTail(node))&&/[\u3400-\u9fff]/.test(node.text)){
   const text=node.text.trim().replace(/\s+/g,' ');if(!['简体中文','日本語'].includes(text)&&!resources.ui[text])missing.add(text);
  }
  ts.forEachChild(node,walk);
 }
 walk(source);
}
if(missing.size){console.error('Missing UI translations:',[...missing]);process.exitCode=1}
else console.log(`V2_UI_LOCALES_OK: ${Object.keys(resources.ui).length} messages`);
