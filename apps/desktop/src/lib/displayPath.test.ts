import { describe, expect, it } from "vitest";
import { displayPath, tildeHome } from "./displayPath";

describe("tildeHome", () => {
  it("shows a Windows home folder as ~", () => {
    expect(tildeHome("C:\\Users\\sam")).toBe("~");
    expect(tildeHome("C:\\Users\\sam\\work\\api")).toBe("~\\work\\api");
    expect(tildeHome("d:/users/Sam Lee/src")).toBe("~/src");
  });

  it("shows a Unix or macOS home folder as ~", () => {
    expect(tildeHome("/home/sam")).toBe("~");
    expect(tildeHome("/home/sam/projects/api")).toBe("~/projects/api");
    expect(tildeHome("/Users/sam/Desktop")).toBe("~/Desktop");
    expect(tildeHome("/root/.config")).toBe("~/.config");
  });

  it("leaves other paths alone, including look-alikes", () => {
    expect(tildeHome("E:\\Projects\\acme-api")).toBe("E:\\Projects\\acme-api");
    expect(tildeHome("C:\\Windows\\System32")).toBe("C:\\Windows\\System32");
    expect(tildeHome("C:\\UsersArchive\\sam")).toBe("C:\\UsersArchive\\sam");
    expect(tildeHome("/homework/sam")).toBe("/homework/sam");
    expect(tildeHome("/srv/app")).toBe("/srv/app");
  });
});

describe("displayPath", () => {
  it("is empty for no path", () => {
    expect(displayPath(undefined)).toBe("");
    expect(displayPath(null)).toBe("");
    expect(displayPath("")).toBe("");
  });

  it("keeps a short path whole", () => {
    expect(displayPath("C:\\Users\\sam\\api")).toBe("~\\api");
    expect(displayPath("/srv/app")).toBe("/srv/app");
  });

  it("shortens a long path in the middle, keeping the last two folders", () => {
    const long = "C:\\Users\\sam\\source\\repos\\company\\platform\\services\\orders-api";
    expect(displayPath(long, 24)).toBe("~\\…\\services\\orders-api");
    expect(displayPath("/var/lib/some/very/deep/tree/of/folders/app/src", 20)).toBe("/…/app/src");
    expect(displayPath("E:\\a-long-folder\\another-long-folder\\third\\leaf", 20)).toBe("E:\\…\\third\\leaf");
  });

  it("does not shorten a path with too few folders to shorten", () => {
    expect(displayPath("E:\\one-very-long-folder-name-indeed", 10)).toBe("E:\\one-very-long-folder-name-indeed");
  });
});
