create table users (
  id uuid primary key,
  provider text not null,
  provider_tenant_id text null,
  provider_user_id text null,
  email text not null,
  display_name text null,
  role text not null default 'verified_user',
  created_at timestamptz not null,
  updated_at timestamptz not null
);

create unique index users_provider_identity_idx
  on users(provider, provider_tenant_id, provider_user_id)
  where provider_tenant_id is not null and provider_user_id is not null;
create unique index users_dev_email_idx
  on users(lower(email))
  where provider = 'dev';
create index users_email_idx on users(lower(email));

create table sessions (
  id uuid primary key,
  user_id uuid not null references users(id) on delete cascade,
  session_token_hash text not null,
  expires_at timestamptz not null,
  created_at timestamptz not null
);

create unique index sessions_token_hash_idx on sessions(session_token_hash);
create index sessions_user_id_idx on sessions(user_id);
create index sessions_expires_at_idx on sessions(expires_at);

create table courses (
  id uuid primary key,
  code text not null,
  title text not null,
  school text null,
  au integer null,
  archived boolean not null default false,
  created_at timestamptz not null,
  updated_at timestamptz not null,
  unique(code)
);

create index courses_code_idx on courses(code);
create index courses_code_lower_idx on courses(lower(code));

create table course_offerings (
  id uuid primary key,
  course_id uuid not null references courses(id) on delete cascade,
  academic_year text not null,
  semester text not null,
  status text not null default 'active',
  inherited_from_offering_id uuid null references course_offerings(id),
  created_at timestamptz not null,
  updated_at timestamptz not null,
  unique(course_id, academic_year, semester)
);

create index course_offerings_lookup_idx
  on course_offerings(course_id, academic_year, semester);
create index course_offerings_inherited_from_idx
  on course_offerings(inherited_from_offering_id);

create table wiki_sections (
  id uuid primary key,
  offering_id uuid not null references course_offerings(id) on delete cascade,
  section_key text not null,
  title text not null,
  head_version_id uuid null,
  inherited_from_section_id uuid null references wiki_sections(id),
  locked boolean not null default false,
  created_at timestamptz not null,
  updated_at timestamptz not null,
  unique(offering_id, section_key)
);

create index wiki_sections_lookup_idx on wiki_sections(offering_id, section_key);
create index wiki_sections_inherited_from_idx on wiki_sections(inherited_from_section_id);

create table wiki_commits (
  id uuid primary key,
  author_user_id uuid not null references users(id),
  message text not null,
  commit_type text not null,
  reverted_commit_id uuid null references wiki_commits(id),
  created_at timestamptz not null
);

create index wiki_commits_created_at_idx on wiki_commits(created_at desc);
create index wiki_commits_author_idx on wiki_commits(author_user_id);

create table wiki_versions (
  id uuid primary key,
  section_id uuid not null references wiki_sections(id) on delete cascade,
  commit_id uuid not null references wiki_commits(id),
  parent_version_id uuid null references wiki_versions(id),
  content_markdown text not null,
  content_json jsonb null,
  content_hash text not null,
  created_at timestamptz not null
);

create index wiki_versions_section_history_idx
  on wiki_versions(section_id, created_at desc);
create index wiki_versions_commit_idx on wiki_versions(commit_id);
create index wiki_versions_parent_idx on wiki_versions(parent_version_id);

alter table wiki_sections
  add constraint wiki_sections_head_version_id_fkey
  foreign key (head_version_id) references wiki_versions(id);

create table wiki_commit_changes (
  id uuid primary key,
  commit_id uuid not null references wiki_commits(id) on delete cascade,
  section_id uuid not null references wiki_sections(id) on delete cascade,
  old_version_id uuid null references wiki_versions(id),
  new_version_id uuid null references wiki_versions(id),
  change_type text not null
);

create index wiki_commit_changes_commit_idx on wiki_commit_changes(commit_id);
create index wiki_commit_changes_section_idx on wiki_commit_changes(section_id);

create table section_verifications (
  id uuid primary key,
  section_id uuid not null references wiki_sections(id) on delete cascade,
  version_id uuid not null references wiki_versions(id) on delete cascade,
  user_id uuid not null references users(id) on delete cascade,
  academic_year text not null,
  semester text not null,
  verification_type text not null,
  created_at timestamptz not null,
  unique(section_id, version_id, user_id, academic_year, semester)
);

create index section_verifications_section_idx on section_verifications(section_id);
create index section_verifications_version_idx on section_verifications(version_id);

create table reviews (
  id uuid primary key,
  offering_id uuid not null references course_offerings(id) on delete cascade,
  user_id uuid not null references users(id) on delete cascade,
  current_version_id uuid null,
  hidden boolean not null default false,
  hidden_reason text null,
  created_at timestamptz not null,
  updated_at timestamptz not null,
  unique(offering_id, user_id)
);

create index reviews_offering_visible_idx on reviews(offering_id, hidden);
create index reviews_user_idx on reviews(user_id);

create table review_versions (
  id uuid primary key,
  review_id uuid not null references reviews(id) on delete cascade,
  author_user_id uuid not null references users(id),
  rating_difficulty integer null,
  rating_workload integer null,
  rating_usefulness integer null,
  rating_teaching integer null,
  workload_hours_per_week integer null,
  body_markdown text not null,
  created_at timestamptz not null
);

create index review_versions_review_history_idx
  on review_versions(review_id, created_at desc);

alter table reviews
  add constraint reviews_current_version_id_fkey
  foreign key (current_version_id) references review_versions(id);

create table moderation_actions (
  id uuid primary key,
  actor_user_id uuid not null references users(id),
  target_type text not null,
  target_id uuid not null,
  action_type text not null,
  reason text null,
  metadata jsonb null,
  created_at timestamptz not null
);

create index moderation_actions_target_idx on moderation_actions(target_type, target_id);
create index moderation_actions_created_at_idx on moderation_actions(created_at desc);

create table reports (
  id uuid primary key,
  reporter_user_id uuid not null references users(id),
  target_type text not null,
  target_id uuid not null,
  reason text not null,
  status text not null default 'open',
  created_at timestamptz not null
);

create index reports_status_created_at_idx on reports(status, created_at desc);
create index reports_target_idx on reports(target_type, target_id);

