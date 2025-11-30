# Terminator Selector Cheat Sheet

> Quick reference guide for building robust UI selectors in Terminator. Selectors follow the pattern `<prefix>:<value>` and can be chained with `>>` to walk the accessibility tree.

| Prefix / Pattern       | Example                                          | What it matches                                                                  | Rough Playwright equivalent\*              |
| ---------------------- | ------------------------------------------------ | -------------------------------------------------------------------------------- | ------------------------------------------ |
| `role:`                | `role:Button`                                    | Elements by accessibility **role** (e.g. `Button`, `Window`, `MenuItem`).        | `role=button`                              |
| `name:`                | `name:Save`                                      | Element whose **accessible name/label** is "Save".                               | `text=Save` or `aria/Save`                 |
| `id:`                  | `id:submit`                                      | Accessibility **ID** (when exposed). On Windows this maps to `AutomationId`.     | `css=#submit`                              |
| `nativeid:`            | `nativeid:42`                                    | **OS-specific automation id** (e.g. Windows `AutomationId`, macOS AXIdentifier). | n/a (desktop-specific)                     |
| `classname:`           | `classname:Edit`                                 | UI **class name** (Win32 `ClassName`, Cocoa `AXRoleDescription`, etc.).          | `css=.Edit`                                |
| `text:`                | `text:Open`                                      | Visible **text content** inside the element.                                     | `text=Open`                                |
| `pos:x,y`              | `pos:100,200`                                    | Element located at **screen coordinates** `(x,y)` (last resort).                 | n/a                                        |
| `visible:true/false`   | `visible:true`                                   | Filter elements by **visibility** on screen.                                     | `:visible` pseudo-class                    |
| `rightof:<sel>`        | `rightof:name:Username`                          | Element **right of** another selector.                                           | `right-of=` locators                       |
| `leftof:<sel>`         | `leftof:role:Checkbox`                           | Element **left of** another selector.                                            | `left-of=` locators                        |
| `above:<sel>`          | `above:name:OK`                                  | Element **above** another selector.                                              | `above=` locators                          |
| `below:<sel>`          | `below:name:OK`                                  | Element **below** another selector.                                              | `below=` locators                          |
| `near:<sel>`           | `near:text:Cancel`                               | Element **near** another selector (within tolerance).                            | `near=` locators                           |
| `nth:<n>`              | `nth:0`                                          | Select the **nth element** (0-based) from matches.                               | `:nth-child(n)`                            |
| `nth-<n>`              | `nth-1`                                          | Select the **nth element from end** (nth-1 = last, nth-2 = second-to-last).      | `:nth-last-child(n)`                       |
| `..`                   | `..`                                             | Navigate to **parent element** (Playwright-style).                               | `xpath=..`                                 |
| `role:<r> && name:<n>` | `role:Button && name:Close`                      | **Compound** selector – role **and** name in one step.                           | `role=button[name="Close"]`                |
| `<selA> >> <selB>`     | `window:Calculator >> role:Button >> name:Seven` | **Chain** selectors to traverse hierarchy, similar to descendant combinators.    | `#Calculator >> role=button[name="Seven"]` |

\* The Playwright column shows an approximate conceptual mapping for web automation. Desktop and web runtimes expose different accessibility trees, so the exact selector semantics may differ.

## Tips

1. Prefer **specific** selectors (e.g. `role:Button && name:Save`) over broad ones (`role:Button`).
2. Build selectors incrementally with `.locator()` chaining to keep them readable and maintainable.
3. Inspect the accessibility tree with the tools mentioned in the main README (Accessibility Insights, Accessibility Inspector, Accerciser) to discover roles and names.
4. Combine positional filters (`rightof:`, `below:`) with role/name for ambiguous layouts.
5. Only fall back to `pos:` or raw `/XPath` when no structured attributes are available.

---

Need more help? Join our [Discord](https://discord.gg/dU9EBuw7Uq) or open an issue!
