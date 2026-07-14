import { readFileSync } from "node:fs"
import { resolve } from "node:path"
import { describe, expect, it } from "vitest"

const workflow = readFileSync(resolve(process.cwd(), ".github/workflows/publish.yml"), "utf8")

describe("Publish workflow", () => {
  it("publishes Windows NSIS and portable ZIP assets for release tags", () => {
    expect(workflow).toContain("platform: windows-latest")
    expect(workflow).toContain('args: "--bundles nsis"')
    expect(workflow).toContain("./scripts/build-gui-portable-windows.ps1")
    expect(workflow).toContain('gh release upload "$RELEASE_TAG" openusage_*_windows_*.zip --clobber')
  })
})
