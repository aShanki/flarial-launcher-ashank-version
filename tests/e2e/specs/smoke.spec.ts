import { browser, $ } from "@wdio/globals";

describe("launcher smoke", () => {
  it("renders the home heading", async () => {
    const heading = await $("h1");
    await heading.waitForExist({ timeout: 10_000 });
    const text = await heading.getText();
    if (!text.includes("Flarial")) throw new Error(`got: ${text}`);
  });

  it("renders the launch button", async () => {
    const button = await $("button");
    await button.waitForExist({ timeout: 5_000 });
    const text = await button.getText();
    if (!text.toLowerCase().includes("launch")) throw new Error(`got: ${text}`);
  });
});
