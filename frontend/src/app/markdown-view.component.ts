import { Component, Input, type OnChanges } from "@angular/core";
import { renderMarkdown } from "./markdown";

@Component({
  selector: "app-markdown-view",
  standalone: true,
  template: '<div class="markdown" [innerHTML]="html"></div>',
})
export class MarkdownViewComponent implements OnChanges {
  @Input() markdown = "";

  html = "";

  ngOnChanges(): void {
    this.html = renderMarkdown(this.markdown);
  }
}
