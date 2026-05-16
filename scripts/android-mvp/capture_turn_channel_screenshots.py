#!/usr/bin/env python3
import asyncio, pathlib
from playwright.async_api import async_playwright

ROOT=pathlib.Path('/Volumes/extension/code/fin')
OUT=ROOT/'reports/android-mvp-screenshots'
OUT.mkdir(parents=True, exist_ok=True)
HTML=(ROOT/'android-client/app/src/main/assets/mobile-shell.html').resolve().as_uri()

async def main():
    async with async_playwright() as p:
        b=await p.chromium.launch()
        page=await b.new_page(viewport={"width":390,"height":844})
        await page.goto(HTML)
        await page.evaluate("""
          S.turns=[{
            u:'normal 用户输入示例',
            a:'normal 助手回复示例',
            control:'',tool:'',closure:'',toolRecords:[],errors:[]
          }];
          S.debugMode=false; renderTurns();
        """)
        await page.screenshot(path=str(OUT/'turn-normal.png'), full_page=True)

        await page.evaluate("""
          S.turns=[{
            u:'debug 用户输入示例',
            a:'debug 助手回复示例',
            control:'continuity-check',
            tool:'provider.call:completed',
            closure:'assistant_message',
            toolRecords:[{tool_name:'provider.call',status:'completed',input_summary:'...',output_summary:'...'}],
            errors:[{tool_name:'session.list',status:'failed',error_summary:'missing runtime_home'}]
          }];
          S.debugMode=true; renderTurns();
        """)
        await page.screenshot(path=str(OUT/'turn-debug.png'), full_page=True)
        await page.screenshot(path=str(OUT/'turn-error-debug.png'), full_page=True)
        await b.close()

asyncio.run(main())
