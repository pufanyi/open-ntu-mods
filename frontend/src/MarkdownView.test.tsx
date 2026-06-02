import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { MarkdownView } from "./MarkdownView";

describe("MarkdownView", () => {
  it("renders safe markdown and strips script tags", () => {
    render(
      <MarkdownView
        markdown={"# Title\n<script>alert('x')</script>\nUseful note"}
      />,
    );
    expect(screen.getByRole("heading", { name: "Title" })).toBeInTheDocument();
    expect(screen.getByText("Useful note")).toBeInTheDocument();
    expect(document.querySelector("script")).not.toBeInTheDocument();
  });
});
