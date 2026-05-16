#!/usr/bin/env python3
import asyncio, json, pathlib
from playwright.async_api import async_playwright
ROOT=pathlib.Path(__file__).resolve().parents[2]
logs=ROOT/'reports'/'android-mvp-logs'; logs.mkdir(parents=True, exist_ok=True)
shots=ROOT/'reports'/'android-mvp-screenshots'; shots.mkdir(parents=True, exist_ok=True)
latest=json.loads((ROOT/'android-client'/'update-dist'/'latest.json').read_text())
html=(ROOT/'android-client'/'app'/'src'/'main'/'assets'/'mobile-shell.html').resolve().as_uri()

async def main():
    async with async_playwright() as p:
        browser=await p.chromium.launch()
        page=await browser.new_page(viewport={"width":390,"height":844})
        await page.add_init_script(f"""
          window.FinMobileBridge = {{
            getWsProfiles: () => JSON.stringify([{{id:'local',name:'Local',endpoint:'ws://127.0.0.1:4040/ws',token:'',project:'fin',expiresAtEpochSec:0}}]),
            readLatestJson: () => JSON.stringify({json.dumps(latest)})
          }};
        """)
        await page.goto(html)
        await page.get_by_role('button', name='Update').click()
        await page.get_by_role('button', name='检查更新').click()
        await page.wait_for_timeout(500)
        txt=await page.locator('#update pre').inner_text()
        (logs/'update-check.log').write_text(txt+'\n')
        await page.screenshot(path=str(shots/'03-update-check.png'), full_page=True)
        await browser.close()

asyncio.run(main())
