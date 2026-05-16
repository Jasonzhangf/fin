#!/usr/bin/env python3
import asyncio, pathlib
from playwright.async_api import async_playwright

OUT=pathlib.Path('reports/android-mvp-screenshots')
OUT.mkdir(parents=True, exist_ok=True)
HTML=pathlib.Path('android-client/app/src/main/assets/mobile-shell.html').resolve().as_uri()

async def main():
    async with async_playwright() as p:
        browser = await p.chromium.launch()
        page = await browser.new_page(viewport={"width":390,"height":844})
        await page.goto(HTML)
        await page.screenshot(path=str(OUT/'01-sessions.png'), full_page=True)
        # 右上：会话入口
        await page.locator('button.round').nth(1).click()
        await page.wait_for_timeout(200)
        await page.screenshot(path=str(OUT/'02-conversation.png'), full_page=True)
        await page.evaluate("closePanel('sessionsPanel')")
        await page.wait_for_timeout(120)

        # 左上：设置入口
        await page.locator('button.round').nth(0).click()
        await page.wait_for_timeout(200)
        await page.locator('#debugToggle').scroll_into_view_if_needed()
        await page.screenshot(path=str(OUT/'02-connection.png'), full_page=True)
        await page.evaluate("closePanel('settingsPanel')")
        await page.wait_for_timeout(120)

        # 对齐原验证产物命名，保留 runtime/update 视图截图占位（主界面）
        await page.screenshot(path=str(OUT/'02-runtime.png'), full_page=True)
        await page.screenshot(path=str(OUT/'02-update.png'), full_page=True)
        await browser.close()

asyncio.run(main())
