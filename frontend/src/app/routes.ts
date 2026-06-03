import {
  Component,
  Input,
  inject,
  isDevMode,
  type OnInit,
} from "@angular/core";
import { FormsModule } from "@angular/forms";
import {
  ActivatedRoute,
  Router,
  RouterLink,
  type Routes,
} from "@angular/router";
import {
  type AccountSession,
  ApiClientError,
  api,
  type Course,
  type CourseOffering,
  type DiffResponse,
  errorMessage,
  type HistoryItem,
  type ModerationAction,
  type OfferingWithCourse,
  type Report,
  type ReviewResponse,
  type SectionDetail,
  type SectionSummary,
  unwrap,
  type WikiVersion,
} from "./api";
import { MarkdownViewComponent } from "./markdown-view.component";
import { SessionService } from "./session.service";

@Component({
  selector: "app-section-header",
  standalone: true,
  template: `
    <div class="section-heading">
      <div>
        <p class="eyebrow">
          {{ detail.course.code }} · {{ detail.offering.academic_year }}
          {{ detail.offering.semester }}
        </p>
        <h1>{{ detail.section.title }}</h1>
        <div class="meta-row">
          @if (detail.section.locked) {
            <span class="badge">Locked</span>
          }
          <span>{{ detail.verification_count }} verification(s)</span>
        </div>
      </div>
    </div>
  `,
})
export class SectionHeaderComponent {
  @Input({ required: true })
  detail!: SectionDetail;
}

@Component({
  selector: "app-review-card",
  standalone: true,
  imports: [MarkdownViewComponent],
  template: `
    <article class="row-card">
      <strong>{{ review.author.display_name || review.author.email }}</strong>
      <span>
        Difficulty {{ review.current_version.rating_difficulty ?? "-" }} ·
        Workload {{ review.current_version.rating_workload ?? "-" }} ·
        Usefulness {{ review.current_version.rating_usefulness ?? "-" }}
      </span>
      <app-markdown-view [markdown]="review.current_version.body_markdown" />
      <small>review {{ review.review.id }}</small>
    </article>
  `,
})
export class ReviewCardComponent {
  @Input({ required: true })
  review!: ReviewResponse;
}

@Component({
  standalone: true,
  imports: [FormsModule, RouterLink],
  template: `
    <section>
      <div class="section-heading">
        <h1>Courses</h1>
        <input
          aria-label="Search courses"
          name="courseSearch"
          [(ngModel)]="search"
          placeholder="Search by code or title"
        />
      </div>
      @if (loading) {
        <p class="muted">Loading...</p>
      }
      @if (error) {
        <p class="error">{{ error }}</p>
      }
      <div class="list">
        @for (course of filteredCourses(); track course.id) {
          <a
            class="row-card"
            [routerLink]="['/courses', course.code]"
          >
            <strong>{{ course.code }}</strong>
            <span>{{ course.title }}</span>
            <small>{{ courseMeta(course) }}</small>
          </a>
        }
      </div>
    </section>
  `,
})
export class HomePageComponent implements OnInit {
  search = "";
  loading = true;
  error = "";
  courses: Course[] = [];

  async ngOnInit(): Promise<void> {
    await this.load();
  }

  async load(): Promise<void> {
    this.loading = true;
    this.error = "";
    try {
      this.courses = unwrap(await api.GET("/api/courses"));
    } catch (error) {
      this.error = errorMessage(error);
    } finally {
      this.loading = false;
    }
  }

  filteredCourses(): Course[] {
    const query = this.search.trim().toLowerCase();
    if (!query) {
      return this.courses;
    }
    return this.courses.filter((course) =>
      `${course.code} ${course.title}`.toLowerCase().includes(query),
    );
  }

  courseMeta(course: Course): string {
    return [course.school, course.au ? `${course.au} AU` : null]
      .filter(Boolean)
      .join(" · ");
  }
}

@Component({
  standalone: true,
  imports: [RouterLink],
  template: `
    <section>
      @if (loading) {
        <p class="muted">Loading...</p>
      }
      @if (error) {
        <p class="error">{{ error }}</p>
      }
      @if (course) {
        <div class="section-heading">
          <div>
            <p class="eyebrow">{{ course.code }}</p>
            <h1>{{ course.title }}</h1>
          </div>
        </div>
      }
      <h2>Offerings</h2>
      <div class="list">
        @for (offering of offerings; track offering.id) {
          <a
            class="row-card"
            [routerLink]="['/offerings', offering.id]"
          >
            <strong>
              {{ offering.academic_year }} {{ offering.semester }}
            </strong>
            <span>{{ offering.status }}</span>
          </a>
        }
      </div>
    </section>
  `,
})
export class CoursePageComponent implements OnInit {
  private readonly route = inject(ActivatedRoute);

  courseCode = requiredParam(this.route, "courseCode");
  loading = true;
  error = "";
  course: Course | null = null;
  offerings: CourseOffering[] = [];

  async ngOnInit(): Promise<void> {
    this.loading = true;
    try {
      const [course, offerings] = await Promise.all([
        api.GET("/api/courses/{code}", {
          params: { path: { code: this.courseCode } },
        }),
        api.GET("/api/courses/{course_ref}/offerings", {
          params: { path: { course_ref: this.courseCode } },
        }),
      ]);
      this.course = unwrap(course);
      this.offerings = unwrap(offerings);
    } catch (error) {
      this.error = errorMessage(error);
    } finally {
      this.loading = false;
    }
  }
}

@Component({
  standalone: true,
  imports: [RouterLink],
  template: `
    <section>
      @if (loading) {
        <p class="muted">Loading...</p>
      }
      @if (error) {
        <p class="error">{{ error }}</p>
      }
      @if (offering) {
        <div class="section-heading">
          <div>
            <p class="eyebrow">{{ offering.course.code }}</p>
            <h1>
              {{ offering.course.title }} ·
              {{ offering.offering.academic_year }}
              {{ offering.offering.semester }}
            </h1>
          </div>
          <a
            class="button"
            [routerLink]="['/offerings', offeringId, 'reviews']"
          >
            Reviews
          </a>
        </div>
      }
      <h2>Wiki Sections</h2>
      <div class="grid">
        @for (summary of sections; track summary.section.id) {
          <a
            class="section-card"
            [routerLink]="['/sections', summary.section.id]"
          >
            <div class="card-title">
              <strong>{{ summary.section.title }}</strong>
              @if (summary.section.locked) {
                <span class="badge">Locked</span>
              }
            </div>
            <p>
              {{
                summary.current_version?.content_markdown?.slice(0, 140) ||
                  "No content yet."
              }}
            </p>
            <small>{{ summary.verification_count }} verification(s)</small>
          </a>
        }
      </div>
      <h2>Review Summary</h2>
      <p>{{ reviews.length }} visible review(s).</p>
    </section>
  `,
})
export class OfferingPageComponent implements OnInit {
  private readonly route = inject(ActivatedRoute);

  offeringId = requiredParam(this.route, "offeringId");
  loading = true;
  error = "";
  offering: OfferingWithCourse | null = null;
  sections: SectionSummary[] = [];
  reviews: ReviewResponse[] = [];

  async ngOnInit(): Promise<void> {
    this.loading = true;
    try {
      const [offering, sections, reviews] = await Promise.all([
        api.GET("/api/offerings/{offering_id}", {
          params: { path: { offering_id: this.offeringId } },
        }),
        api.GET("/api/offerings/{offering_id}/sections", {
          params: { path: { offering_id: this.offeringId } },
        }),
        api.GET("/api/offerings/{offering_id}/reviews", {
          params: { path: { offering_id: this.offeringId } },
        }),
      ]);
      this.offering = unwrap(offering);
      this.sections = unwrap(sections);
      this.reviews = unwrap(reviews);
    } catch (error) {
      this.error = errorMessage(error);
    } finally {
      this.loading = false;
    }
  }
}

@Component({
  standalone: true,
  imports: [RouterLink, MarkdownViewComponent, SectionHeaderComponent],
  template: `
    <section>
      @if (loading) {
        <p class="muted">Loading...</p>
      }
      @if (error) {
        <p class="error">{{ error }}</p>
      }
      @if (section) {
        <app-section-header [detail]="section" />
        <div class="action-row">
          <a class="button" [routerLink]="['/sections', sectionId, 'edit']">
            Edit
          </a>
          <a
            class="button secondary"
            [routerLink]="['/sections', sectionId, 'history']"
          >
            History
          </a>
          <button
            type="button"
            [disabled]="!session.user() || !section.current_version || verifying"
            (click)="verify()"
          >
            Still accurate
          </button>
        </div>
        @if (verifyError) {
          <p class="error">{{ verifyError }}</p>
        }
        <app-markdown-view
          [markdown]="section.current_version?.content_markdown || ''"
        />
      }
    </section>
  `,
})
export class SectionPageComponent implements OnInit {
  private readonly route = inject(ActivatedRoute);
  readonly session = inject(SessionService);

  sectionId = requiredParam(this.route, "sectionId");
  loading = true;
  verifying = false;
  error = "";
  verifyError = "";
  section: SectionDetail | null = null;

  async ngOnInit(): Promise<void> {
    await this.load();
  }

  async load(): Promise<void> {
    this.loading = true;
    try {
      this.section = unwrap(
        await api.GET("/api/sections/{section_id}", {
          params: { path: { section_id: this.sectionId } },
        }),
      );
    } catch (error) {
      this.error = errorMessage(error);
    } finally {
      this.loading = false;
    }
  }

  async verify(): Promise<void> {
    const versionId = this.section?.current_version?.id;
    if (!versionId) {
      return;
    }
    this.verifying = true;
    this.verifyError = "";
    try {
      await api.POST("/api/sections/{section_id}/verify", {
        params: { path: { section_id: this.sectionId } },
        body: { version_id: versionId, verification_type: "still_accurate" },
      });
      await this.load();
    } catch (error) {
      this.verifyError = errorMessage(error);
    } finally {
      this.verifying = false;
    }
  }
}

@Component({
  standalone: true,
  imports: [
    FormsModule,
    RouterLink,
    MarkdownViewComponent,
    SectionHeaderComponent,
  ],
  template: `
    <section>
      @if (loading) {
        <p class="muted">Loading...</p>
      }
      @if (error) {
        <p class="error">{{ error }}</p>
      }
      @if (section) {
        <app-section-header [detail]="section" />
        <form class="editor" (ngSubmit)="save()">
          <label>
            Edit summary
            <input name="message" [(ngModel)]="message" />
          </label>
          <label>
            Markdown
            <textarea name="markdown" [(ngModel)]="markdown"></textarea>
          </label>
          @if (conflict) {
            <div class="conflict">
              <p>{{ conflict }}</p>
              <button type="button" (click)="loadLatest()">Load latest</button>
            </div>
          }
          @if (saveError) {
            <p class="error">{{ saveError }}</p>
          }
          <div class="action-row">
            <button type="submit" [disabled]="saving">Save</button>
            <a
              class="button secondary"
              [routerLink]="['/sections', sectionId]"
            >
              Cancel
            </a>
          </div>
        </form>
        <h2>Preview</h2>
        <app-markdown-view [markdown]="markdown" />
      }
    </section>
  `,
})
export class SectionEditPageComponent implements OnInit {
  private readonly route = inject(ActivatedRoute);
  private readonly router = inject(Router);

  sectionId = requiredParam(this.route, "sectionId");
  loading = true;
  saving = false;
  error = "";
  saveError = "";
  conflict = "";
  conflictVersion: Pick<WikiVersion, "id" | "content_markdown"> | null = null;
  section: SectionDetail | null = null;
  markdown = "";
  message = "Updated section";
  baseVersionId: string | null = null;

  async ngOnInit(): Promise<void> {
    try {
      this.section = unwrap(
        await api.GET("/api/sections/{section_id}", {
          params: { path: { section_id: this.sectionId } },
        }),
      );
      this.baseVersionId = this.section.current_version?.id ?? null;
      this.markdown = this.section.current_version?.content_markdown ?? "";
    } catch (error) {
      this.error = errorMessage(error);
    } finally {
      this.loading = false;
    }
  }

  async save(): Promise<void> {
    this.saving = true;
    this.saveError = "";
    this.conflict = "";
    try {
      unwrap(
        await api.POST("/api/sections/{section_id}/edit", {
          params: { path: { section_id: this.sectionId } },
          body: {
            base_version_id: this.baseVersionId,
            content_markdown: this.markdown,
            content_json: null,
            message: this.message,
          },
        }),
      );
      await this.router.navigate(["/sections", this.sectionId]);
    } catch (error) {
      if (error instanceof ApiClientError && error.status === 409) {
        this.conflictVersion = extractCurrentVersion(error.payload);
        this.conflict = this.conflictVersion
          ? `Current version is now ${this.conflictVersion.id}. Review the latest content before submitting.`
          : "The section changed while you were editing.";
      } else {
        this.saveError = errorMessage(error);
      }
    } finally {
      this.saving = false;
    }
  }

  loadLatest(): void {
    const latest =
      this.conflictVersion ?? this.section?.current_version ?? null;
    this.baseVersionId = latest?.id ?? null;
    this.markdown = latest?.content_markdown ?? "";
    this.conflict = "";
    this.saveError = "";
  }
}

@Component({
  standalone: true,
  imports: [FormsModule, RouterLink, MarkdownViewComponent],
  template: `
    <section>
      <div class="section-heading">
        <h1>History</h1>
        <a class="button secondary" [routerLink]="['/sections', sectionId]">
          Section
        </a>
      </div>
      @if (loading) {
        <p class="muted">Loading...</p>
      }
      @if (error) {
        <p class="error">{{ error }}</p>
      }
      <div class="compare">
        <label>
          Old
          <select name="oldId" [(ngModel)]="oldId">
            <option value="">Select version</option>
            @for (item of history; track item.version.id) {
              <option [value]="item.version.id">{{ item.commit.message }}</option>
            }
          </select>
        </label>
        <label>
          New
          <select name="newId" [(ngModel)]="newId">
            <option value="">Select version</option>
            @for (item of history; track item.version.id) {
              <option [value]="item.version.id">{{ item.commit.message }}</option>
            }
          </select>
        </label>
        <button type="button" (click)="loadDiff()">Compare</button>
      </div>
      @if (diffLoading) {
        <p class="muted">Loading diff...</p>
      }
      @if (diffError) {
        <p class="error">{{ diffError }}</p>
      }
      @if (diff) {
        <div class="diff">
          @for (line of diff.lines; track $index) {
            <pre [class]="'diff-line ' + line.kind">{{ linePrefix(line.kind) }} {{ line.text }}</pre>
          }
        </div>
      }
      @if (selectedVersion(); as selected) {
        <div class="version-preview">
          <div class="section-heading compact">
            <div>
              <p class="eyebrow">
                {{ formatDate(selected.version.created_at) }}
              </p>
              <h2>Version preview</h2>
            </div>
            <small>version {{ selected.version.id }}</small>
          </div>
          <app-markdown-view [markdown]="selected.version.content_markdown" />
        </div>
      }
      <div class="list">
        @for (item of history; track item.version.id) {
          <article class="row-card">
            <strong>{{ item.commit.message }}</strong>
            <span>
              {{ item.commit.commit_type }} by
              {{ item.author.display_name || item.author.email }}
            </span>
            <small>
              version {{ item.version.id }} · commit {{ item.commit.id }}
            </small>
            <div class="action-row">
              <button
                class="secondary"
                type="button"
                (click)="previewId = item.version.id"
              >
                View
              </button>
            </div>
          </article>
        }
      </div>
    </section>
  `,
})
export class SectionHistoryPageComponent implements OnInit {
  private readonly route = inject(ActivatedRoute);

  sectionId = requiredParam(this.route, "sectionId");
  loading = true;
  diffLoading = false;
  error = "";
  diffError = "";
  oldId = "";
  newId = "";
  previewId = "";
  history: HistoryItem[] = [];
  diff: DiffResponse | null = null;

  async ngOnInit(): Promise<void> {
    try {
      this.history = unwrap(
        await api.GET("/api/sections/{section_id}/history", {
          params: { path: { section_id: this.sectionId } },
        }),
      );
      this.previewId = this.history[0]?.version.id ?? "";
    } catch (error) {
      this.error = errorMessage(error);
    } finally {
      this.loading = false;
    }
  }

  async loadDiff(): Promise<void> {
    if (!this.oldId || !this.newId) {
      this.diff = null;
      return;
    }
    this.diffLoading = true;
    this.diffError = "";
    try {
      this.diff = unwrap(
        await api.GET("/api/versions/{old_version_id}/diff/{new_version_id}", {
          params: {
            path: { old_version_id: this.oldId, new_version_id: this.newId },
          },
        }),
      );
    } catch (error) {
      this.diffError = errorMessage(error);
    } finally {
      this.diffLoading = false;
    }
  }

  selectedVersion(): HistoryItem | null {
    return (
      this.history.find((item) => item.version.id === this.previewId) ??
      this.history[0] ??
      null
    );
  }

  linePrefix(kind: string): string {
    if (kind === "added") {
      return "+";
    }
    if (kind === "removed") {
      return "-";
    }
    return " ";
  }

  formatDate(value: string): string {
    return new Date(value).toLocaleString();
  }
}

@Component({
  standalone: true,
  imports: [FormsModule, RouterLink, ReviewCardComponent],
  template: `
    <section>
      <div class="section-heading">
        <h1>Reviews</h1>
        <a
          class="button secondary"
          [routerLink]="['/offerings', offeringId]"
        >
          Offering
        </a>
      </div>
      @if (loading) {
        <p class="muted">Loading...</p>
      }
      @if (error) {
        <p class="error">{{ error }}</p>
      }
      @if (session.user()) {
        <form class="review-form" (ngSubmit)="saveReview()">
          <h2>{{ myReview() ? "Edit your review" : "Create your review" }}</h2>
          <label>
            Difficulty
            <input
              name="difficulty"
              type="number"
              min="1"
              max="5"
              [(ngModel)]="difficulty"
            />
          </label>
          <label>
            Workload
            <input
              name="workload"
              type="number"
              min="1"
              max="5"
              [(ngModel)]="workload"
            />
          </label>
          <label>
            Usefulness
            <input
              name="usefulness"
              type="number"
              min="1"
              max="5"
              [(ngModel)]="usefulness"
            />
          </label>
          <label>
            Teaching
            <input
              name="teaching"
              type="number"
              min="1"
              max="5"
              [(ngModel)]="teaching"
            />
          </label>
          <label>
            Hours per week
            <input
              name="hours"
              type="number"
              min="0"
              max="80"
              [(ngModel)]="hours"
            />
          </label>
          <label>
            Review
            <textarea name="body" [(ngModel)]="body"></textarea>
          </label>
          @if (saveError) {
            <p class="error">{{ saveError }}</p>
          }
          <button type="submit" [disabled]="saving">Save review</button>
        </form>
      } @else {
        <p><a routerLink="/login">Login</a> to create a review.</p>
      }
      <div class="list">
        @for (review of reviews; track review.review.id) {
          <app-review-card [review]="review" />
        }
      </div>
    </section>
  `,
})
export class ReviewsPageComponent implements OnInit {
  private readonly route = inject(ActivatedRoute);
  readonly session = inject(SessionService);

  offeringId = requiredParam(this.route, "offeringId");
  loading = true;
  saving = false;
  error = "";
  saveError = "";
  reviews: ReviewResponse[] = [];
  body = "";
  difficulty = 3;
  workload = 3;
  usefulness = 4;
  teaching = 3;
  hours = 8;

  async ngOnInit(): Promise<void> {
    await this.load();
  }

  async load(): Promise<void> {
    this.loading = true;
    try {
      this.reviews = unwrap(
        await api.GET("/api/offerings/{offering_id}/reviews", {
          params: { path: { offering_id: this.offeringId } },
        }),
      );
      this.populateReviewForm();
    } catch (error) {
      this.error = errorMessage(error);
    } finally {
      this.loading = false;
    }
  }

  myReview(): ReviewResponse | null {
    const userId = this.session.user()?.id;
    if (!userId) {
      return null;
    }
    return (
      this.reviews.find((review) => review.review.user_id === userId) ?? null
    );
  }

  populateReviewForm(): void {
    const review = this.myReview();
    if (!review || this.body) {
      return;
    }
    this.body = review.current_version.body_markdown;
    this.difficulty = review.current_version.rating_difficulty ?? 3;
    this.workload = review.current_version.rating_workload ?? 3;
    this.usefulness = review.current_version.rating_usefulness ?? 4;
    this.teaching = review.current_version.rating_teaching ?? 3;
    this.hours = review.current_version.workload_hours_per_week ?? 8;
  }

  async saveReview(): Promise<void> {
    this.saving = true;
    this.saveError = "";
    const payload = {
      rating_difficulty: Number(this.difficulty),
      rating_workload: Number(this.workload),
      rating_usefulness: Number(this.usefulness),
      rating_teaching: Number(this.teaching),
      workload_hours_per_week: Number(this.hours),
      body_markdown: this.body,
    };

    try {
      const review = this.myReview();
      if (review) {
        unwrap(
          await api.PUT("/api/reviews/{review_id}", {
            params: { path: { review_id: review.review.id } },
            body: payload,
          }),
        );
      } else {
        unwrap(
          await api.POST("/api/reviews", {
            body: { offering_id: this.offeringId, ...payload },
          }),
        );
      }
      await this.load();
    } catch (error) {
      this.saveError = errorMessage(error);
    } finally {
      this.saving = false;
    }
  }
}

@Component({
  standalone: true,
  imports: [FormsModule],
  template: `
    <section>
      <h1>Admin</h1>
      @if (session.loading()) {
        <p class="muted">Loading...</p>
      } @else if (!canAccess()) {
        <p>Trusted editor, moderator, or admin access is required.</p>
      } @else {
        <div class="admin-panel">
          <label>
            Reason
            <input name="reason" [(ngModel)]="reason" />
          </label>
          <label>
            Commit ID
            <input name="commitId" [(ngModel)]="commitId" />
          </label>
          <button type="button" (click)="runAction('revert')">
            Revert commit
          </button>
          <label>
            Section ID
            <input name="sectionId" [(ngModel)]="sectionId" />
          </label>
          <label>
            Version ID
            <input name="versionId" [(ngModel)]="versionId" />
          </label>
          <div class="action-row">
            <button type="button" (click)="runAction('restore')">
              Restore version
            </button>
            <button type="button" (click)="runAction('lock')">
              Lock section
            </button>
            <button type="button" (click)="runAction('unlock')">
              Unlock section
            </button>
          </div>
          <label>
            Review ID
            <input name="reviewId" [(ngModel)]="reviewId" />
          </label>
          <div class="action-row">
            <button type="button" (click)="runAction('hide')">
              Hide review
            </button>
            <button type="button" (click)="runAction('restore-review')">
              Restore review
            </button>
          </div>
          @if (actionError) {
            <p class="error">{{ actionError }}</p>
          }
        </div>
        <h2>Reports</h2>
        @if (loadingReports) {
          <p class="muted">Loading...</p>
        }
        @if (reportsError) {
          <p class="error">{{ reportsError }}</p>
        }
        <div class="list">
          @for (report of reports; track report.id) {
            <article class="row-card">
              <strong>{{ report.target_type }}</strong>
              <span>{{ report.reason }}</span>
              <small>{{ report.status }} · {{ report.target_id }}</small>
              <button type="button" (click)="resolve(report)">Resolve</button>
            </article>
          }
        </div>
        <h2>Audit Log</h2>
        @if (loadingAudit) {
          <p class="muted">Loading...</p>
        }
        @if (auditError) {
          <p class="error">{{ auditError }}</p>
        }
        <div class="list">
          @for (item of audit; track item.id) {
            <article class="row-card">
              <strong>{{ item.action_type }}</strong>
              <span>{{ item.target_type }} · {{ item.target_id }}</span>
              <small>{{ item.reason }}</small>
            </article>
          }
        </div>
      }
    </section>
  `,
})
export class AdminPageComponent implements OnInit {
  readonly session = inject(SessionService);

  commitId = "";
  sectionId = "";
  versionId = "";
  reviewId = "";
  reason = "moderation action";
  actionError = "";
  reportsError = "";
  auditError = "";
  loadingReports = false;
  loadingAudit = false;
  reports: Report[] = [];
  audit: ModerationAction[] = [];

  async ngOnInit(): Promise<void> {
    await this.session.load();
    if (this.canAccess()) {
      await this.loadReports();
    }
    if (this.canAdmin()) {
      await this.loadAudit();
    }
  }

  canAccess(): boolean {
    return roleRank(this.session.user()?.role) >= roleRank("trusted_editor");
  }

  canAdmin(): boolean {
    return roleRank(this.session.user()?.role) >= roleRank("admin");
  }

  async runAction(name: string): Promise<void> {
    this.actionError = "";
    try {
      switch (name) {
        case "revert":
          unwrap(
            await api.POST("/api/admin/commits/{commit_id}/revert", {
              params: { path: { commit_id: this.commitId } },
              body: { reason: this.reason },
            }),
          );
          break;
        case "restore":
          unwrap(
            await api.POST(
              "/api/admin/sections/{section_id}/restore-version/{version_id}",
              {
                params: {
                  path: {
                    section_id: this.sectionId,
                    version_id: this.versionId,
                  },
                },
                body: { reason: this.reason },
              },
            ),
          );
          break;
        case "lock":
          unwrap(
            await api.POST("/api/admin/sections/{section_id}/lock", {
              params: { path: { section_id: this.sectionId } },
              body: { reason: this.reason },
            }),
          );
          break;
        case "unlock":
          unwrap(
            await api.POST("/api/admin/sections/{section_id}/unlock", {
              params: { path: { section_id: this.sectionId } },
            }),
          );
          break;
        case "hide":
          unwrap(
            await api.POST("/api/admin/reviews/{review_id}/hide", {
              params: { path: { review_id: this.reviewId } },
              body: { reason: this.reason },
            }),
          );
          break;
        case "restore-review":
          unwrap(
            await api.POST("/api/admin/reviews/{review_id}/restore", {
              params: { path: { review_id: this.reviewId } },
            }),
          );
          break;
        default:
          throw new Error("Unknown action");
      }
      await this.loadReports();
      if (this.canAdmin()) {
        await this.loadAudit();
      }
    } catch (error) {
      this.actionError = errorMessage(error);
    }
  }

  async loadReports(): Promise<void> {
    this.loadingReports = true;
    try {
      this.reports = unwrap(await api.GET("/api/admin/reports"));
    } catch (error) {
      this.reportsError = errorMessage(error);
    } finally {
      this.loadingReports = false;
    }
  }

  async loadAudit(): Promise<void> {
    this.loadingAudit = true;
    try {
      this.audit = unwrap(await api.GET("/api/admin/audit-log"));
    } catch (error) {
      this.auditError = errorMessage(error);
    } finally {
      this.loadingAudit = false;
    }
  }

  async resolve(report: Report): Promise<void> {
    try {
      unwrap(
        await api.POST("/api/admin/reports/{report_id}/resolve", {
          params: { path: { report_id: report.id } },
          body: { reason: this.reason },
        }),
      );
      await this.loadReports();
      if (this.canAdmin()) {
        await this.loadAudit();
      }
    } catch (error) {
      this.actionError = errorMessage(error);
    }
  }
}

@Component({
  standalone: true,
  imports: [FormsModule],
  template: `
    <section class="login">
      <h1>{{ mode === "register" ? "Register" : "Login" }}</h1>
      <form (ngSubmit)="submitAuth()">
        <div class="tabs" role="tablist" aria-label="Auth mode">
          <button
            [class.secondary]="mode !== 'login'"
            type="button"
            (click)="switchMode('login')"
          >
            Login
          </button>
          <button
            [class.secondary]="mode !== 'register'"
            type="button"
            (click)="switchMode('register')"
          >
            Register
          </button>
        </div>
        <label>
          Email
          <input
            name="email"
            type="email"
            [(ngModel)]="email"
            (ngModelChange)="codeSent = false; code = ''"
            placeholder="you@e.ntu.edu.sg"
            autocomplete="email"
          />
        </label>
        @if (mode === "register") {
          <label>
            Display name
            <input
              name="displayName"
              [(ngModel)]="displayName"
              placeholder="Optional"
              autocomplete="name"
            />
          </label>
        }
        @if (codeSent) {
          <label>
            6-digit code
            <input
              name="code"
              inputmode="numeric"
              pattern="[0-9]{6}"
              [(ngModel)]="code"
              placeholder="123456"
              autocomplete="one-time-code"
            />
          </label>
          <p class="muted">Code sent. It expires in 10 minutes.</p>
        }
        @if (authError) {
          <p class="error">{{ authError }}</p>
        }
        <div class="action-row">
          <button type="submit" [disabled]="authPending">
            {{ codeSent ? (mode === "register" ? "Create account" : "Login") : "Send code" }}
          </button>
          @if (codeSent) {
            <button
              class="secondary"
              type="button"
              [disabled]="authPending"
              (click)="sendCode()"
            >
              Resend
            </button>
          }
        </div>
      </form>
      @if (devMode) {
        <form (ngSubmit)="devLogin()">
          <h2>Dev login</h2>
          <label>
            Dev email
            <input name="devEmail" [(ngModel)]="email" />
          </label>
          <label>
            Display name
            <input name="devDisplayName" [(ngModel)]="displayName" />
          </label>
          <label>
            Role
            <select name="role" [(ngModel)]="role">
              <option value="verified_user">verified_user</option>
              <option value="trusted_editor">trusted_editor</option>
              <option value="moderator">moderator</option>
              <option value="admin">admin</option>
            </select>
          </label>
          @if (devError) {
            <p class="error">{{ devError }}</p>
          }
          <button type="submit" [disabled]="devPending">Login as dev user</button>
        </form>
      }
    </section>
  `,
})
export class LoginPageComponent {
  private readonly router = inject(Router);
  private readonly session = inject(SessionService);

  readonly devMode = isDevMode();
  mode: "login" | "register" = "login";
  email = "";
  displayName = "";
  code = "";
  role = "verified_user";
  codeSent = false;
  authPending = false;
  devPending = false;
  authError = "";
  devError = "";

  switchMode(mode: "login" | "register"): void {
    this.mode = mode;
    this.code = "";
    this.codeSent = false;
    this.authError = "";
  }

  async submitAuth(): Promise<void> {
    if (this.codeSent) {
      await this.verifyCode();
    } else {
      await this.sendCode();
    }
  }

  async sendCode(): Promise<void> {
    this.authPending = true;
    this.authError = "";
    try {
      if (this.mode === "register") {
        unwrap(
          await api.POST("/auth/register/start", {
            body: { email: this.email },
          }),
        );
      } else {
        unwrap(
          await api.POST("/auth/login/start", {
            body: { email: this.email },
          }),
        );
      }
      this.code = "";
      this.codeSent = true;
    } catch (error) {
      this.authError = errorMessage(error);
    } finally {
      this.authPending = false;
    }
  }

  async verifyCode(): Promise<void> {
    this.authPending = true;
    this.authError = "";
    try {
      if (this.mode === "register") {
        const response = unwrap(
          await api.POST("/auth/register/verify", {
            body: {
              email: this.email,
              code: this.code,
              display_name: this.displayName.trim() ? this.displayName : null,
            },
          }),
        );
        this.session.setUser(response.user);
      } else {
        const response = unwrap(
          await api.POST("/auth/login/verify", {
            body: { email: this.email, code: this.code },
          }),
        );
        this.session.setUser(response.user);
      }
      await this.router.navigate(["/"]);
    } catch (error) {
      this.authError = errorMessage(error);
    } finally {
      this.authPending = false;
    }
  }

  async devLogin(): Promise<void> {
    this.devPending = true;
    this.devError = "";
    try {
      const response = unwrap(
        await api.POST("/auth/dev-login", {
          body: {
            email: this.email,
            display_name: this.displayName,
            role: this.role,
          },
        }),
      );
      this.session.setUser(response.user);
      await this.router.navigate(["/"]);
    } catch (error) {
      this.devError = errorMessage(error);
    } finally {
      this.devPending = false;
    }
  }
}

@Component({
  standalone: true,
  imports: [FormsModule, RouterLink],
  template: `
    <section>
      @if (session.loading()) {
        <p class="muted">Loading...</p>
      } @else if (!session.user()) {
        <h1>Account</h1>
        <p><a routerLink="/login">Login</a> to manage your account.</p>
      } @else {
        <div class="section-heading">
          <div>
            <p class="eyebrow">{{ session.user()?.role }}</p>
            <h1>Account</h1>
          </div>
        </div>
        <div class="account-grid">
          <form (ngSubmit)="updateProfile()">
            <h2>Profile</h2>
            <p class="muted">{{ session.user()?.email }} is your account ID.</p>
            <label>
              Display name
              <input
                name="accountDisplayName"
                [(ngModel)]="displayName"
                autocomplete="name"
              />
            </label>
            @if (profileError) {
              <p class="error">{{ profileError }}</p>
            }
            <button type="submit" [disabled]="savingProfile">Save profile</button>
          </form>
          <div class="account-panel">
            <h2>Sessions</h2>
            @if (loadingSessions) {
              <p class="muted">Loading...</p>
            }
            @if (sessionsError) {
              <p class="error">{{ sessionsError }}</p>
            }
            <div class="list">
              @for (session of sessions; track session.id) {
                <article class="row-card">
                  <strong>Active session</strong>
                  <small>
                    Created {{ formatDate(session.created_at) }} · Expires
                    {{ formatDate(session.expires_at) }}
                  </small>
                </article>
              }
            </div>
            @if (logoutError) {
              <p class="error">{{ logoutError }}</p>
            }
            <button
              class="secondary"
              type="button"
              [disabled]="loggingOutAll"
              (click)="logoutAll()"
            >
              Logout all sessions
            </button>
          </div>
        </div>
      }
    </section>
  `,
})
export class AccountPageComponent implements OnInit {
  private readonly router = inject(Router);
  readonly session = inject(SessionService);

  displayName = "";
  loadingSessions = false;
  savingProfile = false;
  loggingOutAll = false;
  profileError = "";
  sessionsError = "";
  logoutError = "";
  sessions: AccountSession[] = [];

  async ngOnInit(): Promise<void> {
    await this.session.load();
    this.displayName = this.session.user()?.display_name ?? "";
    if (this.session.user()) {
      await this.loadSessions();
    }
  }

  async loadSessions(): Promise<void> {
    this.loadingSessions = true;
    try {
      this.sessions = unwrap(await api.GET("/api/account/sessions"));
    } catch (error) {
      this.sessionsError = errorMessage(error);
    } finally {
      this.loadingSessions = false;
    }
  }

  async updateProfile(): Promise<void> {
    this.savingProfile = true;
    this.profileError = "";
    try {
      unwrap(
        await api.PUT("/api/account/profile", {
          body: {
            display_name: this.displayName.trim() ? this.displayName : null,
          },
        }),
      );
      await this.session.refresh();
    } catch (error) {
      this.profileError = errorMessage(error);
    } finally {
      this.savingProfile = false;
    }
  }

  async logoutAll(): Promise<void> {
    this.loggingOutAll = true;
    this.logoutError = "";
    try {
      const result = await api.POST("/api/account/logout-all");
      if (result.error) {
        throw new ApiClientError(result.response.status, result.error);
      }
      await this.session.refresh();
      await this.router.navigate(["/login"]);
    } catch (error) {
      this.logoutError = errorMessage(error);
    } finally {
      this.loggingOutAll = false;
    }
  }

  formatDate(value: string): string {
    return new Date(value).toLocaleString();
  }
}

function requiredParam(route: ActivatedRoute, name: string): string {
  const value = route.snapshot.paramMap.get(name);
  if (!value) {
    throw new Error(`Missing route parameter: ${name}`);
  }
  return value;
}

function extractCurrentVersion(
  payload: unknown,
): Pick<WikiVersion, "id" | "content_markdown"> | null {
  if (!payload || typeof payload !== "object" || !("error" in payload)) {
    return null;
  }
  const error = payload.error as {
    details?: { current_version?: { version?: unknown } };
  };
  const version = error.details?.current_version?.version;
  if (
    version &&
    typeof version === "object" &&
    "id" in version &&
    "content_markdown" in version &&
    typeof version.id === "string" &&
    typeof version.content_markdown === "string"
  ) {
    return {
      id: version.id,
      content_markdown: version.content_markdown,
    };
  }
  return null;
}

function roleRank(role: string | null | undefined): number {
  if (!role) {
    return -1;
  }
  return (
    {
      reader: 0,
      verified_user: 1,
      trusted_editor: 2,
      moderator: 3,
      admin: 4,
      owner: 5,
    }[role] ?? -1
  );
}

export const routes: Routes = [
  { path: "", component: HomePageComponent },
  { path: "courses/:courseCode", component: CoursePageComponent },
  { path: "offerings/:offeringId", component: OfferingPageComponent },
  { path: "sections/:sectionId", component: SectionPageComponent },
  { path: "sections/:sectionId/edit", component: SectionEditPageComponent },
  {
    path: "sections/:sectionId/history",
    component: SectionHistoryPageComponent,
  },
  { path: "offerings/:offeringId/reviews", component: ReviewsPageComponent },
  { path: "admin", component: AdminPageComponent },
  { path: "login", component: LoginPageComponent },
  { path: "account", component: AccountPageComponent },
  { path: "**", redirectTo: "" },
];
