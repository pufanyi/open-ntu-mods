import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  createRootRoute,
  createRoute,
  createRouter,
  Link,
  Outlet,
  useNavigate,
} from "@tanstack/react-router";
import { type FormEvent, useEffect, useMemo, useState } from "react";
import {
  ApiClientError,
  api,
  errorMessage,
  type HistoryItem,
  type ReviewResponse,
  type SectionDetail,
  unwrap,
} from "./api";
import { MarkdownView } from "./MarkdownView";

function Layout() {
  const me = useMe();
  const queryClient = useQueryClient();
  const logout = useMutation({
    mutationFn: async () => {
      await api.POST("/auth/logout");
    },
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["me"] }),
  });

  return (
    <div className="app-shell">
      <header className="topbar">
        <Link to="/" className="brand">
          Open NTU Mods
        </Link>
        <nav className="nav">
          <Link to="/">Courses</Link>
          <Link to="/admin">Admin</Link>
          {me.data?.user ? (
            <button type="button" onClick={() => logout.mutate()}>
              Logout
            </button>
          ) : (
            <Link to="/login">Login</Link>
          )}
        </nav>
      </header>
      {me.data?.user && (
        <div className="session-strip">
          {me.data.user.display_name ?? me.data.user.email} ·{" "}
          {me.data.user.role}
        </div>
      )}
      <main className="content">
        <Outlet />
      </main>
    </div>
  );
}

function HomePage() {
  const [search, setSearch] = useState("");
  const courses = useQuery({
    queryKey: ["courses"],
    queryFn: async () => unwrap(await api.GET("/api/courses")),
  });
  const filtered = (courses.data ?? []).filter((course) =>
    `${course.code} ${course.title}`
      .toLowerCase()
      .includes(search.toLowerCase()),
  );

  return (
    <section>
      <div className="section-heading">
        <h1>Courses</h1>
        <input
          aria-label="Search courses"
          value={search}
          onChange={(event) => setSearch(event.target.value)}
          placeholder="Search by code or title"
        />
      </div>
      <QueryState query={courses} />
      <div className="list">
        {filtered.map((course) => (
          <Link
            className="row-card"
            key={course.id}
            to="/courses/$courseCode"
            params={{ courseCode: course.code }}
          >
            <strong>{course.code}</strong>
            <span>{course.title}</span>
            <small>
              {[course.school, course.au ? `${course.au} AU` : null]
                .filter(Boolean)
                .join(" · ")}
            </small>
          </Link>
        ))}
      </div>
    </section>
  );
}

function CoursePage() {
  const { courseCode } = courseRoute.useParams();
  const course = useQuery({
    queryKey: ["course", courseCode],
    queryFn: async () =>
      unwrap(
        await api.GET("/api/courses/{code}", {
          params: { path: { code: courseCode } },
        }),
      ),
  });
  const offerings = useQuery({
    queryKey: ["course-offerings", courseCode],
    queryFn: async () =>
      unwrap(
        await api.GET("/api/courses/{course_ref}/offerings", {
          params: { path: { course_ref: courseCode } },
        }),
      ),
  });

  return (
    <section>
      <QueryState query={course} />
      {course.data && (
        <div className="section-heading">
          <div>
            <p className="eyebrow">{course.data.code}</p>
            <h1>{course.data.title}</h1>
          </div>
        </div>
      )}
      <h2>Offerings</h2>
      <QueryState query={offerings} />
      <div className="list">
        {(offerings.data ?? []).map((offering) => (
          <Link
            className="row-card"
            key={offering.id}
            to="/offerings/$offeringId"
            params={{ offeringId: offering.id }}
          >
            <strong>
              {offering.academic_year} {offering.semester}
            </strong>
            <span>{offering.status}</span>
          </Link>
        ))}
      </div>
    </section>
  );
}

function OfferingPage() {
  const { offeringId } = offeringRoute.useParams();
  const offering = useOffering(offeringId);
  const sections = useQuery({
    queryKey: ["offering-sections", offeringId],
    queryFn: async () =>
      unwrap(
        await api.GET("/api/offerings/{offering_id}/sections", {
          params: { path: { offering_id: offeringId } },
        }),
      ),
  });
  const reviews = useReviews(offeringId);

  return (
    <section>
      <QueryState query={offering} />
      {offering.data && (
        <div className="section-heading">
          <div>
            <p className="eyebrow">{offering.data.course.code}</p>
            <h1>
              {offering.data.course.title} ·{" "}
              {offering.data.offering.academic_year}{" "}
              {offering.data.offering.semester}
            </h1>
          </div>
          <Link
            className="button"
            to="/offerings/$offeringId/reviews"
            params={{ offeringId }}
          >
            Reviews
          </Link>
        </div>
      )}
      <h2>Wiki Sections</h2>
      <QueryState query={sections} />
      <div className="grid">
        {(sections.data ?? []).map((summary) => (
          <Link
            className="section-card"
            key={summary.section.id}
            to="/sections/$sectionId"
            params={{ sectionId: summary.section.id }}
          >
            <div className="card-title">
              <strong>{summary.section.title}</strong>
              {summary.section.locked && <Badge label="Locked" />}
            </div>
            <p>
              {summary.current_version?.content_markdown.slice(0, 140) ??
                "No content yet."}
            </p>
            <small>{summary.verification_count} verification(s)</small>
          </Link>
        ))}
      </div>
      <h2>Review Summary</h2>
      <QueryState query={reviews} />
      <p>{reviews.data?.length ?? 0} visible review(s).</p>
    </section>
  );
}

function SectionPage() {
  const { sectionId } = sectionRoute.useParams();
  const queryClient = useQueryClient();
  const section = useSection(sectionId);
  const me = useMe();
  const verify = useMutation({
    mutationFn: async (versionId: string) =>
      unwrap(
        await api.POST("/api/sections/{section_id}/verify", {
          params: { path: { section_id: sectionId } },
          body: { version_id: versionId, verification_type: "still_accurate" },
        }),
      ),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["section", sectionId] });
    },
  });

  return (
    <section>
      <QueryState query={section} />
      {section.data && (
        <>
          <SectionHeader detail={section.data} />
          <div className="action-row">
            <Link
              className="button"
              to="/sections/$sectionId/edit"
              params={{ sectionId }}
            >
              Edit
            </Link>
            <Link
              className="button secondary"
              to="/sections/$sectionId/history"
              params={{ sectionId }}
            >
              History
            </Link>
            <button
              type="button"
              disabled={
                !me.data?.user ||
                !section.data.current_version ||
                verify.isPending
              }
              onClick={() => {
                const versionId = section.data?.current_version?.id;
                if (versionId) {
                  verify.mutate(versionId);
                }
              }}
            >
              Still accurate
            </button>
          </div>
          {verify.error && (
            <p className="error">{errorMessage(verify.error)}</p>
          )}
          <MarkdownView
            markdown={section.data.current_version?.content_markdown ?? ""}
          />
        </>
      )}
    </section>
  );
}

function SectionEditPage() {
  const { sectionId } = editRoute.useParams();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const section = useSection(sectionId);
  const [markdown, setMarkdown] = useState("");
  const [message, setMessage] = useState("Updated section");
  const [baseVersionId, setBaseVersionId] = useState<string | null>(null);
  const [conflict, setConflict] = useState<string | null>(null);

  useEffect(() => {
    if (section.data && baseVersionId === null && markdown === "") {
      setBaseVersionId(section.data.current_version?.id ?? null);
      setMarkdown(section.data.current_version?.content_markdown ?? "");
    }
  }, [baseVersionId, markdown, section.data]);

  const mutation = useMutation({
    mutationFn: async () =>
      unwrap(
        await api.POST("/api/sections/{section_id}/edit", {
          params: { path: { section_id: sectionId } },
          body: {
            base_version_id: baseVersionId,
            content_markdown: markdown,
            content_json: null,
            message,
          },
        }),
      ),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["section", sectionId] });
      await queryClient.invalidateQueries({
        queryKey: ["section-history", sectionId],
      });
      navigate({ to: "/sections/$sectionId", params: { sectionId } });
    },
    onError: (error) => {
      if (error instanceof ApiClientError && error.status === 409) {
        const current = extractCurrentVersion(error.payload);
        setConflict(
          current
            ? `Current version is now ${current.id}. Review the latest content before submitting.`
            : "The section changed while you were editing.",
        );
      }
    },
  });

  return (
    <section>
      <QueryState query={section} />
      {section.data && (
        <>
          <SectionHeader detail={section.data} />
          <form
            className="editor"
            onSubmit={(event) => {
              event.preventDefault();
              setConflict(null);
              mutation.mutate();
            }}
          >
            <label>
              Edit summary
              <input
                value={message}
                onChange={(event) => setMessage(event.target.value)}
              />
            </label>
            <label>
              Markdown
              <textarea
                value={markdown}
                onChange={(event) => setMarkdown(event.target.value)}
              />
            </label>
            {conflict && (
              <div className="conflict">
                <p>{conflict}</p>
                <button
                  type="button"
                  onClick={() => {
                    const latest = section.data?.current_version;
                    setBaseVersionId(latest?.id ?? null);
                    setMarkdown(latest?.content_markdown ?? "");
                    setConflict(null);
                  }}
                >
                  Load latest
                </button>
              </div>
            )}
            {mutation.error && !conflict && (
              <p className="error">{errorMessage(mutation.error)}</p>
            )}
            <div className="action-row">
              <button type="submit" disabled={mutation.isPending}>
                Save
              </button>
              <Link
                className="button secondary"
                to="/sections/$sectionId"
                params={{ sectionId }}
              >
                Cancel
              </Link>
            </div>
          </form>
          <h2>Preview</h2>
          <MarkdownView markdown={markdown} />
        </>
      )}
    </section>
  );
}

function SectionHistoryPage() {
  const { sectionId } = historyRoute.useParams();
  const [oldId, setOldId] = useState("");
  const [newId, setNewId] = useState("");
  const [previewId, setPreviewId] = useState("");
  const history = useQuery({
    queryKey: ["section-history", sectionId],
    queryFn: async () =>
      unwrap(
        await api.GET("/api/sections/{section_id}/history", {
          params: { path: { section_id: sectionId } },
        }),
      ),
  });
  const diff = useQuery({
    queryKey: ["diff", oldId, newId],
    enabled: Boolean(oldId && newId),
    queryFn: async () =>
      unwrap(
        await api.GET("/api/versions/{old_version_id}/diff/{new_version_id}", {
          params: { path: { old_version_id: oldId, new_version_id: newId } },
        }),
      ),
  });
  const selectedVersion = useMemo(
    () =>
      (history.data ?? []).find((item) => item.version.id === previewId) ??
      history.data?.[0],
    [history.data, previewId],
  );

  return (
    <section>
      <div className="section-heading">
        <h1>History</h1>
        <Link
          className="button secondary"
          to="/sections/$sectionId"
          params={{ sectionId }}
        >
          Section
        </Link>
      </div>
      <QueryState query={history} />
      <VersionCompareControls
        history={history.data ?? []}
        oldId={oldId}
        newId={newId}
        setOldId={setOldId}
        setNewId={setNewId}
      />
      <QueryState query={diff} />
      {diff.data && (
        <div className="diff">
          {diff.data.lines.map((line) => (
            <pre
              className={`diff-line ${line.kind}`}
              key={`${line.kind}-${line.text}`}
            >
              {line.kind === "added"
                ? "+"
                : line.kind === "removed"
                  ? "-"
                  : " "}{" "}
              {line.text}
            </pre>
          ))}
        </div>
      )}
      {selectedVersion && (
        <div className="version-preview">
          <div className="section-heading compact">
            <div>
              <p className="eyebrow">
                {new Date(selectedVersion.version.created_at).toLocaleString()}
              </p>
              <h2>Version preview</h2>
            </div>
            <small>version {selectedVersion.version.id}</small>
          </div>
          <MarkdownView markdown={selectedVersion.version.content_markdown} />
        </div>
      )}
      <div className="list">
        {(history.data ?? []).map((item) => (
          <article className="row-card" key={item.version.id}>
            <strong>{item.commit.message}</strong>
            <span>
              {item.commit.commit_type} by{" "}
              {item.author.display_name ?? item.author.email}
            </span>
            <small>
              version {item.version.id} · commit {item.commit.id}
            </small>
            <div className="action-row">
              <button
                className="secondary"
                type="button"
                onClick={() => setPreviewId(item.version.id)}
              >
                View
              </button>
            </div>
          </article>
        ))}
      </div>
    </section>
  );
}

function ReviewsPage() {
  const { offeringId } = reviewsRoute.useParams();
  const queryClient = useQueryClient();
  const me = useMe();
  const reviews = useReviews(offeringId);
  const myReview = useMemo(
    () =>
      reviews.data?.find(
        (review) => review.review.user_id === me.data?.user?.id,
      ),
    [reviews.data, me.data?.user?.id],
  );
  const [body, setBody] = useState("");
  const [difficulty, setDifficulty] = useState(3);
  const [workload, setWorkload] = useState(3);
  const [usefulness, setUsefulness] = useState(4);
  const [teaching, setTeaching] = useState(3);
  const [hours, setHours] = useState(8);

  useEffect(() => {
    if (myReview && body === "") {
      setBody(myReview.current_version.body_markdown);
      setDifficulty(myReview.current_version.rating_difficulty ?? 3);
      setWorkload(myReview.current_version.rating_workload ?? 3);
      setUsefulness(myReview.current_version.rating_usefulness ?? 4);
      setTeaching(myReview.current_version.rating_teaching ?? 3);
      setHours(myReview.current_version.workload_hours_per_week ?? 8);
    }
  }, [body, myReview]);

  const mutation = useMutation({
    mutationFn: async () => {
      const payload = {
        rating_difficulty: difficulty,
        rating_workload: workload,
        rating_usefulness: usefulness,
        rating_teaching: teaching,
        workload_hours_per_week: hours,
        body_markdown: body,
      };
      if (myReview) {
        return unwrap(
          await api.PUT("/api/reviews/{review_id}", {
            params: { path: { review_id: myReview.review.id } },
            body: payload,
          }),
        );
      }
      return unwrap(
        await api.POST("/api/reviews", {
          body: { offering_id: offeringId, ...payload },
        }),
      );
    },
    onSuccess: () =>
      queryClient.invalidateQueries({
        queryKey: ["offering-reviews", offeringId],
      }),
  });

  return (
    <section>
      <div className="section-heading">
        <h1>Reviews</h1>
        <Link
          className="button secondary"
          to="/offerings/$offeringId"
          params={{ offeringId }}
        >
          Offering
        </Link>
      </div>
      <QueryState query={reviews} />
      {me.data?.user ? (
        <form
          className="review-form"
          onSubmit={(event: FormEvent) => {
            event.preventDefault();
            mutation.mutate();
          }}
        >
          <h2>{myReview ? "Edit your review" : "Create your review"}</h2>
          <RatingInput
            label="Difficulty"
            value={difficulty}
            setValue={setDifficulty}
          />
          <RatingInput
            label="Workload"
            value={workload}
            setValue={setWorkload}
          />
          <RatingInput
            label="Usefulness"
            value={usefulness}
            setValue={setUsefulness}
          />
          <RatingInput
            label="Teaching"
            value={teaching}
            setValue={setTeaching}
          />
          <label>
            Hours per week
            <input
              type="number"
              min={0}
              max={80}
              value={hours}
              onChange={(event) => setHours(Number(event.target.value))}
            />
          </label>
          <label>
            Review
            <textarea
              value={body}
              onChange={(event) => setBody(event.target.value)}
            />
          </label>
          {mutation.error && (
            <p className="error">{errorMessage(mutation.error)}</p>
          )}
          <button type="submit" disabled={mutation.isPending}>
            Save review
          </button>
        </form>
      ) : (
        <p>
          <Link to="/login">Login</Link> to create a review.
        </p>
      )}
      <div className="list">
        {(reviews.data ?? []).map((review) => (
          <ReviewCard key={review.review.id} review={review} />
        ))}
      </div>
    </section>
  );
}

function AdminPage() {
  const me = useMe();
  const queryClient = useQueryClient();
  const [commitId, setCommitId] = useState("");
  const [sectionId, setSectionId] = useState("");
  const [versionId, setVersionId] = useState("");
  const [reviewId, setReviewId] = useState("");
  const [reason, setReason] = useState("moderation action");
  const reports = useQuery({
    queryKey: ["admin-reports"],
    queryFn: async () => unwrap(await api.GET("/api/admin/reports")),
    enabled: Boolean(
      me.data?.user &&
        roleRank(me.data.user.role) >= roleRank("trusted_editor"),
    ),
  });
  const audit = useQuery({
    queryKey: ["admin-audit"],
    queryFn: async () => unwrap(await api.GET("/api/admin/audit-log")),
    enabled: Boolean(
      me.data?.user && roleRank(me.data.user.role) >= roleRank("admin"),
    ),
  });
  const action = useMutation({
    mutationFn: async (name: string) => {
      switch (name) {
        case "revert":
          return unwrap(
            await api.POST("/api/admin/commits/{commit_id}/revert", {
              params: { path: { commit_id: commitId } },
              body: { reason },
            }),
          );
        case "restore":
          return unwrap(
            await api.POST(
              "/api/admin/sections/{section_id}/restore-version/{version_id}",
              {
                params: {
                  path: { section_id: sectionId, version_id: versionId },
                },
                body: { reason },
              },
            ),
          );
        case "lock":
          return unwrap(
            await api.POST("/api/admin/sections/{section_id}/lock", {
              params: { path: { section_id: sectionId } },
              body: { reason },
            }),
          );
        case "unlock":
          return unwrap(
            await api.POST("/api/admin/sections/{section_id}/unlock", {
              params: { path: { section_id: sectionId } },
            }),
          );
        case "hide":
          return unwrap(
            await api.POST("/api/admin/reviews/{review_id}/hide", {
              params: { path: { review_id: reviewId } },
              body: { reason },
            }),
          );
        case "restore-review":
          return unwrap(
            await api.POST("/api/admin/reviews/{review_id}/restore", {
              params: { path: { review_id: reviewId } },
            }),
          );
        default:
          throw new Error("Unknown action");
      }
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["admin-reports"] });
      queryClient.invalidateQueries({ queryKey: ["admin-audit"] });
    },
  });

  if (
    !me.data?.user ||
    roleRank(me.data.user.role) < roleRank("trusted_editor")
  ) {
    return (
      <section>
        <h1>Admin</h1>
        <p>Trusted editor, moderator, or admin access is required.</p>
      </section>
    );
  }

  return (
    <section>
      <h1>Admin</h1>
      <div className="admin-panel">
        <label>
          Reason
          <input
            value={reason}
            onChange={(event) => setReason(event.target.value)}
          />
        </label>
        <label>
          Commit ID
          <input
            value={commitId}
            onChange={(event) => setCommitId(event.target.value)}
          />
        </label>
        <button type="button" onClick={() => action.mutate("revert")}>
          Revert commit
        </button>
        <label>
          Section ID
          <input
            value={sectionId}
            onChange={(event) => setSectionId(event.target.value)}
          />
        </label>
        <label>
          Version ID
          <input
            value={versionId}
            onChange={(event) => setVersionId(event.target.value)}
          />
        </label>
        <div className="action-row">
          <button type="button" onClick={() => action.mutate("restore")}>
            Restore version
          </button>
          <button type="button" onClick={() => action.mutate("lock")}>
            Lock section
          </button>
          <button type="button" onClick={() => action.mutate("unlock")}>
            Unlock section
          </button>
        </div>
        <label>
          Review ID
          <input
            value={reviewId}
            onChange={(event) => setReviewId(event.target.value)}
          />
        </label>
        <div className="action-row">
          <button type="button" onClick={() => action.mutate("hide")}>
            Hide review
          </button>
          <button type="button" onClick={() => action.mutate("restore-review")}>
            Restore review
          </button>
        </div>
        {action.error && <p className="error">{errorMessage(action.error)}</p>}
      </div>
      <h2>Reports</h2>
      <QueryState query={reports} />
      <div className="list">
        {(reports.data ?? []).map((report) => (
          <article className="row-card" key={report.id}>
            <strong>{report.target_type}</strong>
            <span>{report.reason}</span>
            <small>
              {report.status} · {report.target_id}
            </small>
            <button
              type="button"
              onClick={async () => {
                await api.POST("/api/admin/reports/{report_id}/resolve", {
                  params: { path: { report_id: report.id } },
                  body: { reason },
                });
                queryClient.invalidateQueries({ queryKey: ["admin-reports"] });
                queryClient.invalidateQueries({ queryKey: ["admin-audit"] });
              }}
            >
              Resolve
            </button>
          </article>
        ))}
      </div>
      <h2>Audit Log</h2>
      <QueryState query={audit} />
      <div className="list">
        {(audit.data ?? []).map((item) => (
          <article className="row-card" key={item.id}>
            <strong>{item.action_type}</strong>
            <span>
              {item.target_type} · {item.target_id}
            </span>
            <small>{item.reason}</small>
          </article>
        ))}
      </div>
    </section>
  );
}

function LoginPage() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [email, setEmail] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [code, setCode] = useState("");
  const [codeSent, setCodeSent] = useState(false);
  const [role, setRole] = useState("verified_user");
  const startEmailLogin = useMutation({
    mutationFn: async () =>
      unwrap(
        await api.POST("/auth/email/start", {
          body: { email },
        }),
      ),
    onSuccess: () => setCodeSent(true),
  });
  const verifyEmailLogin = useMutation({
    mutationFn: async () =>
      unwrap(
        await api.POST("/auth/email/verify", {
          body: {
            email,
            code,
            display_name: displayName.trim() ? displayName : null,
          },
        }),
      ),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["me"] });
      navigate({ to: "/" });
    },
  });
  const devLogin = useMutation({
    mutationFn: async () =>
      unwrap(
        await api.POST("/auth/dev-login", {
          body: { email, display_name: displayName, role },
        }),
      ),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["me"] });
      navigate({ to: "/" });
    },
  });

  return (
    <section className="login">
      <h1>Login</h1>
      <form
        onSubmit={(event) => {
          event.preventDefault();
          if (codeSent) {
            verifyEmailLogin.mutate();
          } else {
            startEmailLogin.mutate();
          }
        }}
      >
        <h2>Email code</h2>
        <label>
          Email
          <input
            type="email"
            value={email}
            onChange={(event) => setEmail(event.target.value)}
            placeholder="you@example.com"
            autoComplete="email"
          />
        </label>
        <label>
          Display name
          <input
            value={displayName}
            onChange={(event) => setDisplayName(event.target.value)}
            placeholder="Optional"
            autoComplete="name"
          />
        </label>
        {codeSent && (
          <label>
            6-digit code
            <input
              inputMode="numeric"
              pattern="[0-9]{6}"
              value={code}
              onChange={(event) => setCode(event.target.value)}
              placeholder="123456"
              autoComplete="one-time-code"
            />
          </label>
        )}
        {codeSent && (
          <p className="muted">
            Code sent. In log delivery mode, check the backend logs.
          </p>
        )}
        {startEmailLogin.error && (
          <p className="error">{errorMessage(startEmailLogin.error)}</p>
        )}
        {verifyEmailLogin.error && (
          <p className="error">{errorMessage(verifyEmailLogin.error)}</p>
        )}
        <div className="action-row">
          <button
            type="submit"
            disabled={startEmailLogin.isPending || verifyEmailLogin.isPending}
          >
            {codeSent ? "Verify and login" : "Send code"}
          </button>
          {codeSent && (
            <button
              className="secondary"
              type="button"
              onClick={() => {
                setCode("");
                startEmailLogin.mutate();
              }}
              disabled={startEmailLogin.isPending}
            >
              Resend
            </button>
          )}
        </div>
      </form>
      {import.meta.env.DEV && (
        <form
          onSubmit={(event) => {
            event.preventDefault();
            devLogin.mutate();
          }}
        >
          <h2>Dev login</h2>
          <label>
            Email
            <input
              value={email}
              onChange={(event) => setEmail(event.target.value)}
            />
          </label>
          <label>
            Display name
            <input
              value={displayName}
              onChange={(event) => setDisplayName(event.target.value)}
            />
          </label>
          <label>
            Role
            <select
              value={role}
              onChange={(event) => setRole(event.target.value)}
            >
              <option value="verified_user">verified_user</option>
              <option value="trusted_editor">trusted_editor</option>
              <option value="moderator">moderator</option>
              <option value="admin">admin</option>
            </select>
          </label>
          {devLogin.error && (
            <p className="error">{errorMessage(devLogin.error)}</p>
          )}
          <button type="submit" disabled={devLogin.isPending}>
            Login as dev user
          </button>
        </form>
      )}
    </section>
  );
}

function SectionHeader({ detail }: { detail: SectionDetail }) {
  return (
    <div className="section-heading">
      <div>
        <p className="eyebrow">
          {detail.course.code} · {detail.offering.academic_year}{" "}
          {detail.offering.semester}
        </p>
        <h1>{detail.section.title}</h1>
        <div className="meta-row">
          {detail.section.locked && <Badge label="Locked" />}
          <span>{detail.verification_count} verification(s)</span>
        </div>
      </div>
    </div>
  );
}

function VersionCompareControls({
  history,
  oldId,
  newId,
  setOldId,
  setNewId,
}: {
  history: HistoryItem[];
  oldId: string;
  newId: string;
  setOldId: (id: string) => void;
  setNewId: (id: string) => void;
}) {
  return (
    <div className="compare">
      <label>
        Old
        <select
          value={oldId}
          onChange={(event) => setOldId(event.target.value)}
        >
          <option value="">Select version</option>
          {history.map((item) => (
            <option value={item.version.id} key={item.version.id}>
              {item.commit.message}
            </option>
          ))}
        </select>
      </label>
      <label>
        New
        <select
          value={newId}
          onChange={(event) => setNewId(event.target.value)}
        >
          <option value="">Select version</option>
          {history.map((item) => (
            <option value={item.version.id} key={item.version.id}>
              {item.commit.message}
            </option>
          ))}
        </select>
      </label>
    </div>
  );
}

function ReviewCard({ review }: { review: ReviewResponse }) {
  return (
    <article className="row-card">
      <strong>{review.author.display_name ?? review.author.email}</strong>
      <span>
        Difficulty {review.current_version.rating_difficulty ?? "-"} · Workload{" "}
        {review.current_version.rating_workload ?? "-"} · Usefulness{" "}
        {review.current_version.rating_usefulness ?? "-"}
      </span>
      <MarkdownView markdown={review.current_version.body_markdown} />
      <small>review {review.review.id}</small>
    </article>
  );
}

function RatingInput({
  label,
  value,
  setValue,
}: {
  label: string;
  value: number;
  setValue: (value: number) => void;
}) {
  return (
    <label>
      {label}
      <input
        type="number"
        min={1}
        max={5}
        value={value}
        onChange={(event) => setValue(Number(event.target.value))}
      />
    </label>
  );
}

function Badge({ label }: { label: string }) {
  return <span className="badge">{label}</span>;
}

function QueryState({
  query,
}: {
  query: { isLoading: boolean; error: unknown };
}) {
  if (query.isLoading) {
    return <p className="muted">Loading...</p>;
  }
  if (query.error) {
    return <p className="error">{errorMessage(query.error)}</p>;
  }
  return null;
}

function useMe() {
  return useQuery({
    queryKey: ["me"],
    queryFn: async () => unwrap(await api.GET("/api/me")),
  });
}

function useOffering(offeringId: string) {
  return useQuery({
    queryKey: ["offering", offeringId],
    queryFn: async () =>
      unwrap(
        await api.GET("/api/offerings/{offering_id}", {
          params: { path: { offering_id: offeringId } },
        }),
      ),
  });
}

function useSection(sectionId: string) {
  return useQuery({
    queryKey: ["section", sectionId],
    queryFn: async () =>
      unwrap(
        await api.GET("/api/sections/{section_id}", {
          params: { path: { section_id: sectionId } },
        }),
      ),
  });
}

function useReviews(offeringId: string) {
  return useQuery({
    queryKey: ["offering-reviews", offeringId],
    queryFn: async () =>
      unwrap(
        await api.GET("/api/offerings/{offering_id}/reviews", {
          params: { path: { offering_id: offeringId } },
        }),
      ),
  });
}

function extractCurrentVersion(
  payload: unknown,
): { id: string; content_markdown: string } | null {
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

function roleRank(role: string): number {
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

const rootRoute = createRootRoute({ component: Layout });
const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: HomePage,
});
const courseRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/courses/$courseCode",
  component: CoursePage,
});
const offeringRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/offerings/$offeringId",
  component: OfferingPage,
});
const sectionRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/sections/$sectionId",
  component: SectionPage,
});
const editRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/sections/$sectionId/edit",
  component: SectionEditPage,
});
const historyRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/sections/$sectionId/history",
  component: SectionHistoryPage,
});
const reviewsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/offerings/$offeringId/reviews",
  component: ReviewsPage,
});
const adminRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/admin",
  component: AdminPage,
});
const loginRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/login",
  component: LoginPage,
});

const routeTree = rootRoute.addChildren([
  indexRoute,
  courseRoute,
  offeringRoute,
  sectionRoute,
  editRoute,
  historyRoute,
  reviewsRoute,
  adminRoute,
  loginRoute,
]);

export const router = createRouter({ routeTree });

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
