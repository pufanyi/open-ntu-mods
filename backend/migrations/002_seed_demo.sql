insert into users (id, provider, provider_tenant_id, provider_user_id, email, display_name, role, created_at, updated_at)
values
  ('00000000-0000-0000-0000-000000000001', 'dev', null, null, 'student@e.ntu.edu.sg', 'Demo Student', 'verified_user', now(), now()),
  ('00000000-0000-0000-0000-000000000002', 'dev', null, null, 'editor@e.ntu.edu.sg', 'Trusted Editor', 'trusted_editor', now(), now()),
  ('00000000-0000-0000-0000-000000000003', 'dev', null, null, 'admin@e.ntu.edu.sg', 'Demo Admin', 'admin', now(), now())
on conflict do nothing;

insert into courses (id, code, title, school, au, archived, created_at, updated_at)
values ('10000000-0000-0000-0000-000000000001', 'SC2001', 'Algorithm Design and Analysis', 'SCSE', 3, false, now(), now())
on conflict (code) do nothing;

insert into course_offerings (id, course_id, academic_year, semester, status, inherited_from_offering_id, created_at, updated_at)
values
  ('20000000-0000-0000-0000-000000000001', '10000000-0000-0000-0000-000000000001', 'AY2024/25', 'Sem 1', 'active', null, now(), now()),
  ('20000000-0000-0000-0000-000000000002', '10000000-0000-0000-0000-000000000001', 'AY2025/26', 'Sem 1', 'active', '20000000-0000-0000-0000-000000000001', now(), now())
on conflict do nothing;

insert into wiki_sections (id, offering_id, section_key, title, head_version_id, inherited_from_section_id, locked, created_at, updated_at)
values
  ('30000000-0000-0000-0000-000000000001', '20000000-0000-0000-0000-000000000001', 'overview', 'Overview', null, null, false, now(), now()),
  ('30000000-0000-0000-0000-000000000002', '20000000-0000-0000-0000-000000000001', 'assessment', 'Assessment', null, null, false, now(), now()),
  ('30000000-0000-0000-0000-000000000003', '20000000-0000-0000-0000-000000000001', 'workload', 'Workload', null, null, false, now(), now()),
  ('30000000-0000-0000-0000-000000000004', '20000000-0000-0000-0000-000000000001', 'project', 'Project', null, null, false, now(), now()),
  ('30000000-0000-0000-0000-000000000005', '20000000-0000-0000-0000-000000000001', 'exam', 'Exam', null, null, false, now(), now()),
  ('30000000-0000-0000-0000-000000000006', '20000000-0000-0000-0000-000000000001', 'tips', 'Tips', null, null, false, now(), now()),
  ('30000000-0000-0000-0000-000000000101', '20000000-0000-0000-0000-000000000002', 'overview', 'Overview', null, '30000000-0000-0000-0000-000000000001', false, now(), now()),
  ('30000000-0000-0000-0000-000000000102', '20000000-0000-0000-0000-000000000002', 'assessment', 'Assessment', null, '30000000-0000-0000-0000-000000000002', false, now(), now())
on conflict do nothing;

insert into wiki_sections (id, offering_id, section_key, title, head_version_id, inherited_from_section_id, locked, created_at, updated_at)
values
  ('30000000-0000-0000-0000-000000000103', '20000000-0000-0000-0000-000000000002', 'workload', 'Workload', null, '30000000-0000-0000-0000-000000000003', false, now(), now()),
  ('30000000-0000-0000-0000-000000000104', '20000000-0000-0000-0000-000000000002', 'project', 'Project', null, '30000000-0000-0000-0000-000000000004', false, now(), now()),
  ('30000000-0000-0000-0000-000000000105', '20000000-0000-0000-0000-000000000002', 'exam', 'Exam', null, '30000000-0000-0000-0000-000000000005', false, now(), now()),
  ('30000000-0000-0000-0000-000000000106', '20000000-0000-0000-0000-000000000002', 'tips', 'Tips', null, '30000000-0000-0000-0000-000000000006', false, now(), now())
on conflict do nothing;

insert into wiki_commits (id, author_user_id, message, commit_type, reverted_commit_id, created_at)
values
  ('40000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000003', 'Seed demo wiki sections', 'edit', null, now()),
  ('40000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000003', 'Set local AY2025/26 overview wording', 'edit', null, now())
on conflict do nothing;

insert into wiki_versions (id, section_id, commit_id, parent_version_id, content_markdown, content_json, content_hash, created_at)
values
  ('50000000-0000-0000-0000-000000000001', '30000000-0000-0000-0000-000000000001', '40000000-0000-0000-0000-000000000001', null, 'SC2001 is a demo course page for discussing algorithm design themes, prerequisites, and student-maintained public information.', null, 'seed-overview', now()),
  ('50000000-0000-0000-0000-000000000002', '30000000-0000-0000-0000-000000000002', '40000000-0000-0000-0000-000000000001', null, 'Assessment details are maintained by students and should be verified against official course communication for the current semester.', null, 'seed-assessment', now()),
  ('50000000-0000-0000-0000-000000000003', '30000000-0000-0000-0000-000000000003', '40000000-0000-0000-0000-000000000001', null, 'Typical workload varies by background. Add semester-specific observations without uploading copyrighted materials.', null, 'seed-workload', now()),
  ('50000000-0000-0000-0000-000000000004', '30000000-0000-0000-0000-000000000004', '40000000-0000-0000-0000-000000000001', null, 'Project information belongs here when it is public and non-copyrighted. Avoid sharing private instructions verbatim.', null, 'seed-project', now()),
  ('50000000-0000-0000-0000-000000000005', '30000000-0000-0000-0000-000000000005', '40000000-0000-0000-0000-000000000001', null, 'Exam notes should describe public structure only. Do not post exam questions or restricted materials.', null, 'seed-exam', now()),
  ('50000000-0000-0000-0000-000000000006', '30000000-0000-0000-0000-000000000006', '40000000-0000-0000-0000-000000000001', null, 'Tips can include study strategies, pacing advice, and links to legal public resources.', null, 'seed-tips', now()),
  ('50000000-0000-0000-0000-000000000101', '30000000-0000-0000-0000-000000000101', '40000000-0000-0000-0000-000000000002', '50000000-0000-0000-0000-000000000001', 'SC2001 AY2025/26 Sem 1 currently inherits most public information from AY2024/25. Update sections when this semester differs.', null, 'seed-2025-overview', now())
on conflict do nothing;

update wiki_sections
set head_version_id = case id
  when '30000000-0000-0000-0000-000000000001' then '50000000-0000-0000-0000-000000000001'
  when '30000000-0000-0000-0000-000000000002' then '50000000-0000-0000-0000-000000000002'
  when '30000000-0000-0000-0000-000000000003' then '50000000-0000-0000-0000-000000000003'
  when '30000000-0000-0000-0000-000000000004' then '50000000-0000-0000-0000-000000000004'
  when '30000000-0000-0000-0000-000000000005' then '50000000-0000-0000-0000-000000000005'
  when '30000000-0000-0000-0000-000000000006' then '50000000-0000-0000-0000-000000000006'
  when '30000000-0000-0000-0000-000000000101' then '50000000-0000-0000-0000-000000000101'
  else head_version_id
end,
updated_at = now()
where id in (
  '30000000-0000-0000-0000-000000000001',
  '30000000-0000-0000-0000-000000000002',
  '30000000-0000-0000-0000-000000000003',
  '30000000-0000-0000-0000-000000000004',
  '30000000-0000-0000-0000-000000000005',
  '30000000-0000-0000-0000-000000000006',
  '30000000-0000-0000-0000-000000000101'
);

insert into wiki_commit_changes (id, commit_id, section_id, old_version_id, new_version_id, change_type)
values
  ('60000000-0000-0000-0000-000000000001', '40000000-0000-0000-0000-000000000001', '30000000-0000-0000-0000-000000000001', null, '50000000-0000-0000-0000-000000000001', 'edit'),
  ('60000000-0000-0000-0000-000000000002', '40000000-0000-0000-0000-000000000001', '30000000-0000-0000-0000-000000000002', null, '50000000-0000-0000-0000-000000000002', 'edit'),
  ('60000000-0000-0000-0000-000000000003', '40000000-0000-0000-0000-000000000001', '30000000-0000-0000-0000-000000000003', null, '50000000-0000-0000-0000-000000000003', 'edit'),
  ('60000000-0000-0000-0000-000000000004', '40000000-0000-0000-0000-000000000001', '30000000-0000-0000-0000-000000000004', null, '50000000-0000-0000-0000-000000000004', 'edit'),
  ('60000000-0000-0000-0000-000000000005', '40000000-0000-0000-0000-000000000001', '30000000-0000-0000-0000-000000000005', null, '50000000-0000-0000-0000-000000000005', 'edit'),
  ('60000000-0000-0000-0000-000000000006', '40000000-0000-0000-0000-000000000001', '30000000-0000-0000-0000-000000000006', null, '50000000-0000-0000-0000-000000000006', 'edit'),
  ('60000000-0000-0000-0000-000000000101', '40000000-0000-0000-0000-000000000002', '30000000-0000-0000-0000-000000000101', '50000000-0000-0000-0000-000000000001', '50000000-0000-0000-0000-000000000101', 'edit')
on conflict do nothing;

insert into section_verifications (id, section_id, version_id, user_id, academic_year, semester, verification_type, created_at)
values ('70000000-0000-0000-0000-000000000001', '30000000-0000-0000-0000-000000000101', '50000000-0000-0000-0000-000000000101', '00000000-0000-0000-0000-000000000001', 'AY2025/26', 'Sem 1', 'still_accurate', now())
on conflict do nothing;

insert into reviews (id, offering_id, user_id, current_version_id, hidden, hidden_reason, created_at, updated_at)
values ('80000000-0000-0000-0000-000000000001', '20000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000001', null, false, null, now(), now())
on conflict do nothing;

insert into review_versions (id, review_id, author_user_id, rating_difficulty, rating_workload, rating_usefulness, rating_teaching, workload_hours_per_week, body_markdown, created_at)
values ('90000000-0000-0000-0000-000000000001', '80000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000001', 4, 4, 4, 3, 8, 'Demo review: useful algorithm practice, with workload depending on prior data structures experience.', now())
on conflict do nothing;

update reviews
set current_version_id = '90000000-0000-0000-0000-000000000001',
    updated_at = now()
where id = '80000000-0000-0000-0000-000000000001'
  and current_version_id is null;
