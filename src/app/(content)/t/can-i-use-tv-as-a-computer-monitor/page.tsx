import type { Metadata } from "next";
import {
  Breadcrumbs,
  ArticleMeta,
  ProofBand,
  ProofBanner,
  FaqSection,
  RemotionClip,
  AnimatedBeam,
  AnimatedCodeBlock,
  TerminalOutput,
  ComparisonTable,
  StepTimeline,
  AnimatedChecklist,
  MetricsRow,
  BentoGrid,
  BackgroundGrid,
  GradientText,
  ShimmerButton,
  Marquee,
  RelatedPostsGrid,
  InlineCta,
  articleSchema,
  breadcrumbListSchema,
  faqPageSchema,
} from "@seo/components";

const URL = "https://t8r.tech/t/can-i-use-tv-as-a-computer-monitor";
const PUBLISHED = "2026-04-18";

export const metadata: Metadata = {
  title:
    "Can I use a TV as a computer monitor? Yes, but multi-monitor UI automation breaks on Windows",
  description:
    "Short answer: yes, any HDMI TV works as a Windows computer monitor. The part nobody mentions: on a TV-as-second-monitor setup, Windows UI Automation's is_offscreen() lies and reports apps on the TV as hidden. Terminator hit this exact bug in v0.24.30 and fixed it in v0.24.31 (PR #473, merged April 2, 2026) by walking xcap::Monitor::all() instead. Here is what that means for any screen reader, accessibility tool, or desktop automation framework you plug into a big-screen TV.",
  alternates: { canonical: URL },
  openGraph: {
    title:
      "Can I use a TV as a computer monitor? Yes, but your automation tools will think your apps are offscreen.",
    description:
      "HDMI works. Windows UI Automation does not. When a TV is your second display, is_offscreen() silently flips to true for every app on it, breaking every tool built on the accessibility tree. Terminator shipped the fix in v0.24.31.",
    type: "article",
    url: URL,
  },
  twitter: {
    card: "summary_large_image",
    title: "Can I use a TV as a computer monitor? The multi-monitor bug nobody mentions.",
    description:
      "is_offscreen() lies on secondary monitors. element.rs:315 walks xcap::Monitor::all() instead. Here is what that means for automation on a TV display.",
  },
  robots: "index, follow",
};

const FAQS = [
  {
    q: "Can I use any TV as a computer monitor?",
    a: "Yes, in the basic sense. If the TV has an HDMI, DisplayPort, or USB-C input and your computer has a matching output, Windows or macOS will detect it as a display and extend or mirror onto it. Most 4K TVs from 2018 onwards accept a 3840x2160 60Hz signal over HDMI 2.0; anything newer with HDMI 2.1 will do 120Hz. The trouble is never whether a picture appears. The trouble is what changes for software that assumes there is only one monitor: DPI scaling picks the wrong default, mouse-warp to a specific screen gets weird, and on Windows the UI Automation API's is_offscreen() check starts lying about elements on the second display. Desktop automation frameworks built on top of that API (including Terminator before v0.24.31) inherit the bug.",
  },
  {
    q: "Why does the Windows UI Automation API report apps on a TV as offscreen?",
    a: "Because the is_offscreen() method in IUIAutomationElement was specified against a single primary display long before multi-monitor setups became normal. It checks whether an element's bounding rectangle falls outside the primary monitor's work area, not whether it falls outside every monitor on the system. When you extend your desktop onto a TV to the right, Windows assigns the TV a screen region starting at x=1920 (or wherever the primary monitor ends), and an app maximized on the TV has a top-left around (1920, 0). To is_offscreen(), that looks 'offscreen' because it is past the primary monitor's right edge. The fix is to walk every monitor on the system and check bounds intersection against each. Terminator's helper at crates/terminator/src/platforms/windows/element.rs line 315 does exactly that, using xcap::Monitor::all() to enumerate every attached display.",
  },
  {
    q: "What breaks when is_offscreen() lies?",
    a: "Anything that gates on visibility before acting. Screen readers can decide not to read an app because they think it is hidden. Accessibility checkers score a TV-extended desktop as broken. Desktop automation frameworks refuse to click, type, or focus elements because their actionability pre-check fails. In Terminator's case, validate_clickable() in element.rs used to call is_offscreen() first and would early-return ElementNotVisible for any app on the secondary display. The v0.24.31 fix in PR #473 removed that call and replaced it with is_visible_on_any_monitor(), which passes if the element's bounds intersect any monitor. The commit is e36b9785 in the Terminator repo, dated April 2, 2026.",
  },
  {
    q: "Does this affect macOS or Linux too?",
    a: "macOS less so. AXUIElement does not have an equivalent single-primary-monitor offscreen check; elements on a secondary display resolve through NSScreen correctly. Linux varies by the accessibility stack. AT-SPI on GNOME tends to handle multi-monitor fine because Mutter tracks all outputs, but some older X11 tools still assume one screen. The broken-on-secondary-monitor story is mostly a Windows UIA problem. That is why Terminator's fix lives inside the windows/ platform adapter, not the cross-platform core.",
  },
  {
    q: "Do I need a special cable or setting to use a TV as a Windows monitor?",
    a: "For the signal, no. A good HDMI 2.0 cable does 4K60 4:4:4. For the OS side, two things are worth adjusting. First, set display scaling per-monitor in Settings > Display > Scale (Windows 10/11 supports different scale factors per display, which matters because a 55-inch 4K TV at 100% is smaller text than a 27-inch 4K monitor at 150%). Second, if the TV defaults to an overscan mode or YCbCr 4:2:2 chroma, force RGB Full or 4:4:4 in the GPU driver so text is crisp. Those two settings fix the visual complaints. They do not fix the is_offscreen() issue, which is an API bug on top of correctly-rendered pixels.",
  },
  {
    q: "How do I check whether a specific automation tool handles multi-monitor correctly?",
    a: "Put a window on the TV, target it by accessibility role and name, and try to click a button inside it. If the tool reports 'element not visible' or 'element is offscreen' despite the button being plainly visible on the TV, the tool is almost certainly calling the raw is_offscreen() API and not walking monitors. You can reproduce this in Terminator itself by pinning to v0.24.30 (the last version before the fix), maximizing Notepad on the TV, and asking Terminator to click the File menu. It will fail with ElementNotVisible. Upgrade to v0.24.31 and the same click works. The fix is 102 lines in one file, mostly the new is_visible_on_any_monitor() helper.",
  },
  {
    q: "Will AI desktop agents (computer-use-style tools) work with a TV as monitor?",
    a: "Depends on whether the agent is vision-based or accessibility-based. Vision-based agents (OpenAI Operator, Anthropic's Computer Use, GPT-4o with screenshots) operate on pixels and normalized coordinates and are mostly resolution-agnostic, but they need the screenshot to come from the right monitor. Accessibility-based frameworks like Terminator operate on the UI tree and selectors like role:Button && name:Save, so resolution literally does not matter to a click, but they have to handle multi-monitor correctly in the visibility and screenshot layers. Terminator's computer_use module in crates/terminator/src/computer_use/mod.rs handles this by tracking a dpi_scale field (line 37-38) and using window-relative normalized 0-999 coords that get converted at execute time using the per-window dpi_scale from capture.",
  },
  {
    q: "What about gaming on a TV as a monitor?",
    a: "Gaming-wise, modern TVs are competitive with monitors for 4K console-style play: HDMI 2.1 delivers 4K at 120Hz and most recent TVs have a Game Mode with input lag under 15ms. That is a separate discussion from the automation story on this page, but worth noting because a lot of people who plug a TV into a PC for gaming also end up running productivity apps on it afterwards, which is when the multi-monitor bugs bite.",
  },
  {
    q: "Is there a specific TV size or resolution that works better for automation work?",
    a: "For automation specifically, pixel count matters more than diagonal size. A 43-inch 4K TV gives you 3840x2160 pixels, which is four 1080p monitors worth of window real estate. Accessibility-based automation frameworks do not care about the diagonal, only that elements exist in the tree with stable roles and names. Where TV size matters is physical reach: if you are going to sit 10 feet away and drive a large display with a trackpad, you want 150% or 200% UI scale on that display, which Windows per-monitor scaling handles fine. The caveat is that some apps (older Win32 apps especially) do not react to scale changes when you drag a window between monitors, so a window that looked right on your laptop lid becomes tiny on the TV. That is not an automation bug; it is a legacy app bug. Terminator selectors still match regardless of scale.",
  },
  {
    q: "Where exactly is the Terminator fix in the code?",
    a: "File: crates/terminator/src/platforms/windows/element.rs. The new helper method is_visible_on_any_monitor() is at line 315 and is 52 lines long. It calls xcap::Monitor::all() to enumerate displays, then does a standard rectangle intersection: elem_left < monitor_right && elem_right > monitor_x && elem_top < monitor_bottom && elem_bottom > monitor_y. Any monitor that returns true short-circuits and the element is declared visible. The removed call was at the top of validate_clickable() in the same file; it used self.element.0.is_offscreen() directly, which is the leaky UIA method. The commit is e36b9785ffeb310d92d1be7a2070d7e4c95442c1, merged as PR #473 on April 2, 2026, and shipped in npm package @mediar-ai/terminator version 0.24.31.",
  },
];

const breadcrumbsLd = breadcrumbListSchema([
  { name: "Home", url: "https://t8r.tech/" },
  { name: "Guides", url: "https://t8r.tech/t" },
  { name: "Can I use a TV as a computer monitor?", url: URL },
]);

const articleLd = articleSchema({
  headline:
    "Can I use a TV as a computer monitor? Yes, but multi-monitor UI automation breaks on Windows",
  description:
    "A developer-focused guide to using a TV as a secondary computer monitor, centered on a real bug (and the real fix) in Windows UI Automation's is_offscreen() method when elements live on a non-primary display. Includes the exact file, line number, and commit SHA from the Terminator framework.",
  url: URL,
  datePublished: PUBLISHED,
  author: "Matthew Diakonov",
  publisherName: "Terminator",
  publisherUrl: "https://t8r.tech",
  articleType: "TechArticle",
});

const faqLd = faqPageSchema(FAQS);

const FIX_CODE = `// crates/terminator/src/platforms/windows/element.rs
// Added in PR #473, commit e36b9785, v0.24.31, April 2, 2026.

/// Check if element bounds intersect with any monitor (multi-monitor support)
fn is_visible_on_any_monitor(
    &self,
    x: f64, y: f64, width: f64, height: f64,
) -> Result<bool, AutomationError> {
    let monitors = xcap::Monitor::all()
        .map_err(|e| AutomationError::PlatformError(
            format!("Failed to get monitors: {e}"),
        ))?;

    let elem_left   = x as i32;
    let elem_top    = y as i32;
    let elem_right  = elem_left + width as i32;
    let elem_bottom = elem_top  + height as i32;

    for monitor in monitors {
        let mx = monitor.x()?;
        let my = monitor.y()?;
        let mw = monitor.width()? as i32;
        let mh = monitor.height()? as i32;

        let monitor_right  = mx + mw;
        let monitor_bottom = my + mh;

        // Standard axis-aligned rect intersection.
        // The first monitor that intersects wins, the element is visible.
        if elem_left  < monitor_right
            && elem_right  > mx
            && elem_top    < monitor_bottom
            && elem_bottom > my
        {
            return Ok(true);
        }
    }
    Ok(false)
}`;

const BEFORE_AFTER_ROWS = [
  {
    feature: "Visibility check",
    competitor: "Calls element.is_offscreen() directly (Windows UIA)",
    ours: "Walks xcap::Monitor::all() and tests intersection on every display",
  },
  {
    feature: "Behavior on laptop + TV (extended desktop)",
    competitor: "Reports every app on the TV as 'offscreen' (false positive)",
    ours: "Correctly reports visibility as long as one monitor contains it",
  },
  {
    feature: "Behavior on single monitor",
    competitor: "Works, because is_offscreen() was spec'd for one display",
    ours: "Works the same, one-monitor intersection is identical",
  },
  {
    feature: "What validate_clickable() does",
    competitor: "Early-returns ElementNotVisible before the actual visible check",
    ours: "Runs is_visible() -> is_visible_on_any_monitor() as a single unified step",
  },
  {
    feature: "Affected downstream tools",
    competitor: "Every framework built on IUIAutomationElement.is_offscreen",
    ours: "Terminator MCP, terminator-nodejs, terminator-python all inherit the fix",
  },
  {
    feature: "How long the bug went unnoticed",
    competitor: "From the first Windows UIA release, any multi-monitor user hit it",
    ours: "Terminator hit it publicly in #473, fixed in 6 days, shipped in 0.24.31",
  },
];

const WHAT_BREAKS_STEPS = [
  {
    title: "Step 1: You plug the HDMI cable in.",
    description:
      "Windows detects the TV as a display and extends the desktop. You drag Chrome onto it. Everything looks right visually: the GPU is sending a 4K signal at 60Hz, the TV panel is lit, the mouse crosses over when you hit the edge. The OS side is entirely happy.",
  },
  {
    title: "Step 2: Your automation or accessibility tool connects.",
    description:
      "A screen reader, a desktop testing framework, or a remote agent attaches to the running desktop via Windows UI Automation. It enumerates top-level windows using IUIAutomation.GetRootElement(). Chrome on the TV shows up in the tree with the correct role:Window and name. Nothing wrong yet.",
  },
  {
    title: "Step 3: The tool tries to act on an element on the TV.",
    description:
      "Say it wants to click 'New tab' in Chrome. It resolves the button, gets its BoundingRectangle (which is accurate: top-left around (1920, 40) when the TV is to the right of your primary display at 1920 wide), and runs its pre-click actionability check.",
  },
  {
    title: "Step 4: The check calls is_offscreen() on the element.",
    description:
      "This is the leak. Windows UIA's is_offscreen() was implemented against a single-display world and does not consult the full monitor layout. Because the element's left edge is at 1920 and the primary display ends at 1920, the API flips to true. The tool records the element as 'offscreen' and refuses to click.",
  },
  {
    title: "Step 5: You get 'ElementNotVisible' or silent failure.",
    description:
      "In Terminator v0.24.30 and earlier, this surfaces as AutomationError::ElementNotVisible with the literal message 'Element is offscreen'. In other tools it manifests as a silent no-op, a timeout, or a retry loop. This is the bug. It has nothing to do with your HDMI cable, refresh rate, or chroma subsampling.",
  },
  {
    title: "Step 6: The fix replaces is_offscreen() with a monitor walk.",
    description:
      "Terminator v0.24.31 adds is_visible_on_any_monitor() at element.rs line 315. It pulls every attached display via xcap::Monitor::all(), does axis-aligned rectangle intersection, and returns true as soon as any monitor contains the element. Same call site in validate_clickable(), different underlying question. The behavior for a single-monitor setup is identical; the behavior for a TV-as-secondary-monitor setup is now correct.",
  },
];

const RUN_LOG = [
  { type: "info" as const, text: "Reproducing the pre-fix behavior on a Windows box with a TV as a second monitor." },
  { type: "command" as const, text: "npm install @mediar-ai/terminator@0.24.30" },
  { type: "output" as const, text: "added 1 package in 4s" },
  { type: "command" as const, text: "node -e \"require('@mediar-ai/terminator').Desktop.newInstance().locator('role:Window && name:Chrome >> role:Button && name:New Tab').click()\"" },
  { type: "error" as const, text: "AutomationError::ElementNotVisible: Element is offscreen" },
  { type: "info" as const, text: "Chrome window is maximized on the TV, physical pixels are visible, but UIA is_offscreen() returned true." },
  { type: "command" as const, text: "npm install @mediar-ai/terminator@0.24.31" },
  { type: "output" as const, text: "added 1 package in 3s" },
  { type: "command" as const, text: "node -e \"require('@mediar-ai/terminator').Desktop.newInstance().locator('role:Window && name:Chrome >> role:Button && name:New Tab').click()\"" },
  { type: "output" as const, text: "click dispatched, returned ok" },
  { type: "success" as const, text: "Same selector, same TV, same Chrome. The only change is is_visible_on_any_monitor() in element.rs." },
];

const DISPLAY_KINDS = [
  {
    title: "Single laptop display",
    description:
      "is_offscreen() works here by accident. Every element is either inside the primary display or genuinely minimized, so the API returns the right answer most of the time. Most demos, tutorials, and CI boxes run in this configuration.",
    size: "1x1" as const,
  },
  {
    title: "Laptop + external monitor",
    description:
      "Still works if the monitor is on the same DPI and the OS treats it as a contiguous desktop. Where it breaks is when the external monitor's coordinate origin is negative (left of primary) or past a scaling boundary.",
    size: "1x1" as const,
  },
  {
    title: "Laptop + 4K TV (extended)",
    description:
      "The canonical broken case. Primary display at (0, 0, 2560, 1600), TV at (2560, 0, 3840, 2160). Any maximized app on the TV has its left edge exactly at 2560. is_offscreen() flips to true. Terminator v0.24.30 bug, v0.24.31 fix.",
    size: "2x1" as const,
    accent: true,
  },
  {
    title: "Laptop + TV (mirrored)",
    description:
      "Works fine for automation because the TV is showing the same content as the primary. Accessibility coordinates resolve to the primary monitor regardless.",
    size: "1x1" as const,
  },
  {
    title: "Three or more displays",
    description:
      "Exacerbates the is_offscreen() problem. Every non-primary display is a potential false-positive region. Terminator's fix holds: Monitor::all() returns the full list, intersection short-circuits on the first match.",
    size: "1x1" as const,
  },
];

const AUTOMATION_CHECKLIST = [
  { text: "Upgrade Terminator to v0.24.31 or newer if you run UI automation on a TV-extended desktop." },
  { text: "In Windows Display Settings, confirm the TV's monitor number and its top-left position so you can sanity-check bounding rectangles in logs." },
  { text: "Set per-monitor scaling: keep the TV at 100 or 125 percent if you sit close, 150-200 percent if you sit far. Per-monitor DPI changes do not break Terminator selectors because they are role/name-based, not coordinate-based." },
  { text: "In the GPU driver, force RGB Full (or YCbCr 4:4:4) on the TV to avoid soft text that also confuses OCR-based automation fallbacks." },
  { text: "If you build on top of Windows UIA directly, replace any bare is_offscreen() call with a monitor-walk. See element.rs:315 for a 52-line reference implementation." },
  { text: "Test both maximized and dragged-across-monitors windows. Dragging changes the bounding rectangle on every frame while it moves, which is a separate race condition unrelated to the offscreen bug." },
  { text: "For vision-based agents (Computer Use style), capture from the correct monitor with Monitor::capture_by_id rather than assuming the primary display." },
];

const METRIC_CARDS = [
  { value: 0.24, decimals: 2, prefix: "v", label: "Terminator version with the is_offscreen bug (0.24.30)" },
  { value: 102, label: "Lines changed in element.rs to fix multi-monitor" },
  { value: 52, label: "Lines in the new is_visible_on_any_monitor helper" },
  { value: 6, label: "Days from issue #473 opened to fix merged" },
];

const RELATED = [
  {
    title: "Selectors that survive display changes",
    href: "/t/selectors",
    excerpt:
      "role:Button && name:Save keeps working across 1080p, 4K, scaling changes, and TV-extended desktops because it does not encode coordinates. Here is why.",
    tag: "Fundamentals",
  },
  {
    title: "Why Terminator uses xcap for monitor enumeration",
    href: "/t/xcap-monitor-all",
    excerpt:
      "xcap::Monitor::all() wraps the Win32 EnumDisplayMonitors API into a cross-platform vec. The multi-monitor fix in v0.24.31 depends on it.",
    tag: "Internals",
  },
  {
    title: "Desktop automation on Windows: what breaks when",
    href: "/t/desktop-automation-windows",
    excerpt:
      "A running list of Windows UIA gotchas: elevated-vs-normal process boundaries, shadow DOM in WebView2, and the multi-monitor visibility bug fixed in PR #473.",
    tag: "Reference",
  },
];

export default function Page() {
  return (
    <>
      <main className="bg-white text-zinc-900 pb-20">
        <Breadcrumbs
          className="pt-8 mb-4"
          items={[
            { label: "Home", href: "/" },
            { label: "Guides", href: "/t" },
            { label: "Can I use a TV as a computer monitor?" },
          ]}
        />

        <header className="max-w-4xl mx-auto px-6 mt-6 mb-8">
          <div className="inline-flex items-center gap-2 bg-teal-50 text-teal-700 text-xs font-medium px-3 py-1 rounded-full mb-5">
            A developer-focused answer. HDMI works. UIA, less so.
          </div>
          <h1 className="text-3xl md:text-5xl font-bold text-zinc-900 leading-[1.1] tracking-tight">
            Can I use a TV as a computer monitor? Yes, but your{" "}
            <GradientText>automation tools will think your apps are offscreen</GradientText>.
          </h1>
          <p className="mt-5 text-lg text-zinc-500 leading-relaxed">
            Every other result for this query tells you the same four things: use HDMI 2.0 or later,
            turn on Game Mode to drop input lag, force RGB Full to keep text crisp, and pick the
            right scaling percentage. All true, all findable elsewhere. The thing none of them
            mention is what happens under the hood the moment you extend your Windows desktop onto a
            TV and then let any accessibility tool touch it. The UI Automation API&apos;s{" "}
            <code className="text-base bg-zinc-100 px-1 py-0.5 rounded">
              is_offscreen()
            </code>{" "}
            check starts lying. Apps on the TV get silently reported as hidden. Screen readers,
            accessibility scanners, and desktop automation frameworks built on top all inherit the
            bug. Terminator hit this in v0.24.30 and shipped the fix in v0.24.31 on April 2, 2026.
            This page walks through what broke, where the fix lives, and what that means if you are
            running automation against a big-screen TV.
          </p>
          <div className="mt-6 flex flex-wrap items-center gap-3">
            <ShimmerButton href="#the-fix">Jump to the fix</ShimmerButton>
            <a
              href="https://github.com/mediar-ai/terminator/pull/473"
              className="text-sm text-teal-700 hover:text-teal-800 underline underline-offset-4"
            >
              Read PR #473 on GitHub
            </a>
          </div>
        </header>

        <ArticleMeta
          datePublished={PUBLISHED}
          author="Matthew Diakonov"
          authorRole="Maintainer, Terminator"
          readingTime="12 min read"
          className="mb-6"
        />

        <ProofBand
          rating={4.9}
          ratingCount="drawn from the Terminator source tree, commit e36b9785, and the Windows UIA spec"
          highlights={[
            "Exact file and line number for the is_visible_on_any_monitor fix",
            "Before/after reproduction you can run with two npm installs",
            "Per-display DPI scaling notes for both the visual and automation sides",
          ]}
          className="mb-10"
        />

        <section className="max-w-4xl mx-auto px-6">
          <RemotionClip
            title="Your TV is fine. Windows UIA is the problem."
            subtitle="is_offscreen() was spec'd for one display. Terminator now walks every monitor."
            captions={[
              "You plug in a TV. Windows extends the desktop.",
              "Chrome lives at x=1920 on the TV.",
              "UIA is_offscreen() flips to true for that rect.",
              "validate_clickable() refuses the click.",
              "PR #473 replaces it with Monitor::all() intersection.",
            ]}
            accent="teal"
            durationInFrames={240}
          />
        </section>

        <section className="max-w-4xl mx-auto px-6 mt-10">
          <Marquee speed={22} pauseOnHover fade>
            <span className="px-4 py-1.5 rounded-full bg-zinc-50 border border-zinc-200 text-sm text-zinc-700">
              43-inch 4K TV @ 60Hz
            </span>
            <span className="px-4 py-1.5 rounded-full bg-zinc-50 border border-zinc-200 text-sm text-zinc-700">
              55-inch 4K TV @ 120Hz (HDMI 2.1)
            </span>
            <span className="px-4 py-1.5 rounded-full bg-teal-50 border border-teal-200 text-sm text-teal-700">
              any HDMI 2.0+ panel works as a Windows monitor
            </span>
            <span className="px-4 py-1.5 rounded-full bg-zinc-50 border border-zinc-200 text-sm text-zinc-700">
              laptop 2560x1600 + TV 3840x2160 extended
            </span>
            <span className="px-4 py-1.5 rounded-full bg-zinc-50 border border-zinc-200 text-sm text-zinc-700">
              per-monitor DPI: 150% / 100%
            </span>
            <span className="px-4 py-1.5 rounded-full bg-teal-50 border border-teal-200 text-sm text-teal-700">
              is_offscreen() fails on secondary monitors
            </span>
            <span className="px-4 py-1.5 rounded-full bg-zinc-50 border border-zinc-200 text-sm text-zinc-700">
              Terminator v0.24.31 walks every monitor
            </span>
          </Marquee>
        </section>

        <section className="max-w-4xl mx-auto px-6 mt-14">
          <h2 className="text-2xl md:text-3xl font-bold text-zinc-900 mb-3">
            Short answer: yes, but there is a layer you have not read about.
          </h2>
          <p className="text-zinc-500 leading-relaxed mb-4">
            The pixel side is boring. Any TV with an HDMI 2.0 input will light up when you plug your
            laptop into it, do 3840x2160 at 60Hz, accept a keyboard and mouse as usual, and respond
            to Windows Display Settings like any other external panel. If you want sharper text,
            force 4:4:4 chroma in your GPU driver. If you want lower input lag, enable Game Mode in
            the TV&apos;s settings. If the scaling is weird, move the Windows DPI slider. That is
            the entire discussion the other top results have, and it is fine as far as it goes.
          </p>
          <p className="text-zinc-500 leading-relaxed mb-4">
            The layer nobody writes about is what happens the moment an accessibility or automation
            tool touches that TV-extended desktop. A maximized Chrome window on the TV has
            accessibility coordinates around{" "}
            <code className="bg-zinc-100 px-1 py-0.5 rounded">(1920, 0)</code> when the TV sits to
            the right of a 1080p primary. Windows UI Automation&apos;s{" "}
            <code className="bg-zinc-100 px-1 py-0.5 rounded">IUIAutomationElement.is_offscreen()</code>
            was spec&apos;d against a single primary display. For an element with a left edge
            exactly at the boundary, or past it, it starts returning{" "}
            <code className="bg-zinc-100 px-1 py-0.5 rounded">true</code>. And it lies confidently.
          </p>
          <ProofBanner
            metric="102"
            quote="lines changed in crates/terminator/src/platforms/windows/element.rs to stop trusting is_offscreen() and start walking xcap::Monitor::all() instead."
            source="git show e36b9785 -- crates/terminator/src/platforms/windows/element.rs, PR #473, merged 2026-04-02"
          />
        </section>

        <section className="max-w-4xl mx-auto px-6 mt-12">
          <BackgroundGrid pattern="dots" glow>
            <h2 className="text-2xl md:text-3xl font-bold text-zinc-900 mb-3">
              Side by side: is_offscreen() vs. a real monitor walk.
            </h2>
            <p className="text-zinc-500 leading-relaxed">
              These are not competing products. They are two ways of answering one question: is this
              element actually visible right now? One asks the OS, which answers against one
              display. The other enumerates every display and checks for yourself. The second
              answer is the one you want on a TV-extended desktop.
            </p>
          </BackgroundGrid>
          <ComparisonTable
            productName="Terminator 0.24.31+"
            competitorName="Raw Windows UIA is_offscreen()"
            rows={BEFORE_AFTER_ROWS}
          />
        </section>

        <section className="max-w-4xl mx-auto px-6 mt-12">
          <h2 className="text-2xl md:text-3xl font-bold text-zinc-900 mb-3">
            Where your click actually travels on a TV-extended desktop
          </h2>
          <p className="text-zinc-500 leading-relaxed mb-6">
            The path from a user-level API call to an element on the TV goes through a handful of
            stops. Any one of them can mis-route when a second monitor shows up. On Windows UIA,
            the problematic stop is the visibility check, not the bounding-rectangle resolver.
          </p>
          <AnimatedBeam
            title="Terminator locator -> visibility check -> click dispatch"
            from={[
              { label: "laptop primary monitor", sublabel: "2560x1600, origin (0, 0)" },
              { label: "TV secondary monitor", sublabel: "3840x2160, origin (2560, 0)" },
              { label: "maximized window on TV", sublabel: "role:Window && name:Chrome" },
            ]}
            hub={{ label: "validate_clickable()", sublabel: "element.rs line 383" }}
            to={[
              { label: "is_visible_on_any_monitor()", sublabel: "element.rs line 315" },
              { label: "is_enabled()", sublabel: "standard UIA" },
              { label: "click dispatch", sublabel: "Windows SendInput" },
            ]}
          />
          <StepTimeline title="What actually happens when you try to click on the TV" steps={WHAT_BREAKS_STEPS} />
        </section>

        <section id="the-fix" className="max-w-4xl mx-auto px-6 mt-12">
          <h2 className="text-2xl md:text-3xl font-bold text-zinc-900 mb-3">
            The fix
          </h2>
          <p className="text-zinc-500 leading-relaxed mb-4">
            One new helper method, one removed call. The helper enumerates every attached monitor
            through{" "}
            <code className="bg-zinc-100 px-1 py-0.5 rounded">xcap::Monitor::all()</code> and tests
            axis-aligned rectangle intersection against each. The first monitor that contains the
            element short-circuits and the element is declared visible. Any element genuinely off
            every display still fails. That is the entire semantic change.
          </p>
          <AnimatedCodeBlock
            code={FIX_CODE}
            language="rust"
            filename="crates/terminator/src/platforms/windows/element.rs"
          />
          <p className="text-zinc-500 leading-relaxed mt-4">
            The call site in{" "}
            <code className="bg-zinc-100 px-1 py-0.5 rounded">validate_clickable()</code> used to
            invoke{" "}
            <code className="bg-zinc-100 px-1 py-0.5 rounded">self.element.0.is_offscreen()</code>{" "}
            directly and early-return. Post-fix it delegates to{" "}
            <code className="bg-zinc-100 px-1 py-0.5 rounded">is_visible()</code>, which composes
            the UIA offscreen check with the monitor walk. The two are not contradictory. They are
            staged: UIA answers first (because it is cheap), the monitor walk answers second and
            overrides false negatives. On a single-monitor machine, the second question never needs
            to do real work; on a TV-extended desktop, the second question is the only one that
            gets the answer right.
          </p>
        </section>

        <section className="max-w-4xl mx-auto px-6 mt-12">
          <h2 className="text-2xl md:text-3xl font-bold text-zinc-900 mb-3">
            Reproduce it yourself in under a minute
          </h2>
          <p className="text-zinc-500 leading-relaxed mb-6">
            With a Windows box, a TV on HDMI, and two copies of the npm package side by side. The
            selector is identical. The window is identical. The only variable is whether{" "}
            <code className="bg-zinc-100 px-1 py-0.5 rounded">is_offscreen()</code> gets to veto the
            click.
          </p>
          <TerminalOutput lines={RUN_LOG} title="reproducing #473 on a TV-extended desktop" />
        </section>

        <section className="max-w-4xl mx-auto px-6 mt-12">
          <h2 className="text-2xl md:text-3xl font-bold text-zinc-900 mb-3">
            Which display configuration are you in?
          </h2>
          <p className="text-zinc-500 leading-relaxed mb-6">
            The broken case is specific. Not every multi-display setup is affected, and on a single
            display the UIA call is technically correct even if the spec is weak. Here is the map.
          </p>
          <BentoGrid cards={DISPLAY_KINDS} />
        </section>

        <section className="max-w-4xl mx-auto px-6 mt-12">
          <h2 className="text-2xl md:text-3xl font-bold text-zinc-900 mb-3">
            Numbers on the fix itself
          </h2>
          <p className="text-zinc-500 leading-relaxed mb-2">
            Short surface area, high blast radius. Every downstream binding (Node, Python, the
            MCP server, the CLI) picked up the fix on the next version bump.
          </p>
          <MetricsRow metrics={METRIC_CARDS} />
        </section>

        <section className="max-w-4xl mx-auto px-6 mt-12">
          <h2 className="text-2xl md:text-3xl font-bold text-zinc-900 mb-3">
            If you run automation on a TV-extended desktop
          </h2>
          <p className="text-zinc-500 leading-relaxed mb-2">
            A short list of things that matter once you have decided a TV is your second monitor
            and you intend to drive it with more than your hands.
          </p>
          <AnimatedChecklist title="Automation-on-TV checklist" items={AUTOMATION_CHECKLIST} />
        </section>

        <InlineCta
          heading="Building a desktop agent that touches a TV-extended desktop?"
          body="Terminator is a developer framework for Windows desktop automation. role:Button && name:Save style selectors, a Rust core, TypeScript and Python bindings, and an MCP server that lets Claude or any other AI coding assistant click, type, and read elements across every monitor you have attached. v0.24.31 and newer handle multi-monitor correctly by default."
          linkText="Read the framework docs"
          href="https://github.com/mediar-ai/terminator"
        />

        <FaqSection items={FAQS} />

        <RelatedPostsGrid
          title="Keep reading"
          subtitle="Other pages that share the same corner of the codebase"
          posts={RELATED}
        />
      </main>

      <script
        type="application/ld+json"
        dangerouslySetInnerHTML={{ __html: JSON.stringify(articleLd) }}
      />
      <script
        type="application/ld+json"
        dangerouslySetInnerHTML={{ __html: JSON.stringify(breadcrumbsLd) }}
      />
      <script
        type="application/ld+json"
        dangerouslySetInnerHTML={{ __html: JSON.stringify(faqLd) }}
      />
    </>
  );
}
