import terminator
import asyncio

async def click_button(element):
    locator = await element.all()
    locator[0].click()

async def main():
    desktop = terminator.Desktop()
    desktop.open_application('calculator')
    seven = desktop.locator('application:Calculator >> button:7')
    await click_button(seven)
    plus = desktop.locator('application:Calculator >> button:Add')
    await click_button(plus)
    three = desktop.locator('application:Calculator >> button:3')
    await click_button(three)
    equal = desktop.locator('application:Calculator >> button:Equals')
    await click_button(equal)

# Control applications programmatically
asyncio.run(main())
# Result: 10 appears in calculator