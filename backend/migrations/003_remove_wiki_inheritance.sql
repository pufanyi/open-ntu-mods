create extension if not exists pgcrypto;

with recursive chain(section_id, current_section_id, depth) as (
  select s.id, s.inherited_from_section_id, 1
  from wiki_sections s
  where s.head_version_id is null
    and s.inherited_from_section_id is not null
  union all
  select chain.section_id, parent.inherited_from_section_id, chain.depth + 1
  from chain
  join wiki_sections parent on parent.id = chain.current_section_id
  where parent.head_version_id is null
    and parent.inherited_from_section_id is not null
    and chain.depth < 32
),
source_versions as (
  select distinct on (chain.section_id)
    chain.section_id,
    parent.head_version_id as source_version_id
  from chain
  join wiki_sections parent on parent.id = chain.current_section_id
  where parent.head_version_id is not null
  order by chain.section_id, chain.depth
),
commit_row as (
  insert into wiki_commits (id, author_user_id, message, commit_type, reverted_commit_id, created_at)
  select
    gen_random_uuid(),
    '00000000-0000-0000-0000-000000000003',
    'Materialize standalone wiki pages',
    'edit',
    null,
    now()
  where exists (select 1 from source_versions)
  returning id
),
inserted_versions as (
  insert into wiki_versions
    (id, section_id, commit_id, parent_version_id, content_markdown, content_json, content_hash, created_at)
  select
    gen_random_uuid(),
    source_versions.section_id,
    commit_row.id,
    source_versions.source_version_id,
    source.content_markdown,
    source.content_json,
    source.content_hash,
    now()
  from source_versions
  join wiki_versions source on source.id = source_versions.source_version_id
  cross join commit_row
  returning id, section_id, parent_version_id
),
updated_sections as (
  update wiki_sections s
  set head_version_id = inserted_versions.id,
      inherited_from_section_id = null,
      updated_at = now()
  from inserted_versions
  where s.id = inserted_versions.section_id
  returning
    s.id as section_id,
    inserted_versions.parent_version_id as old_version_id,
    inserted_versions.id as new_version_id
)
insert into wiki_commit_changes
  (id, commit_id, section_id, old_version_id, new_version_id, change_type)
select
  gen_random_uuid(),
  commit_row.id,
  updated_sections.section_id,
  updated_sections.old_version_id,
  updated_sections.new_version_id,
  'materialize'
from updated_sections
cross join commit_row;

with target as (
  select s.id as section_id, s.head_version_id as old_version_id
  from wiki_sections s
  join wiki_versions v on v.id = s.head_version_id
  where s.id = '30000000-0000-0000-0000-000000000101'
    and v.content_markdown like '%inherits%'
),
commit_row as (
  insert into wiki_commits (id, author_user_id, message, commit_type, reverted_commit_id, created_at)
  select
    gen_random_uuid(),
    '00000000-0000-0000-0000-000000000003',
    'Refresh standalone AY2025/26 overview',
    'edit',
    null,
    now()
  where exists (select 1 from target)
  returning id
),
inserted_version as (
  insert into wiki_versions
    (id, section_id, commit_id, parent_version_id, content_markdown, content_json, content_hash, created_at)
  select
    gen_random_uuid(),
    target.section_id,
    commit_row.id,
    target.old_version_id,
    'SC2001 AY2025/26 Sem 1 is a standalone demo course page. Students can edit this page and use history to view earlier versions.',
    null,
    encode(digest('SC2001 AY2025/26 Sem 1 is a standalone demo course page. Students can edit this page and use history to view earlier versions.' || chr(0), 'sha256'), 'hex'),
    now()
  from target
  cross join commit_row
  returning id, section_id, parent_version_id
),
updated_section as (
  update wiki_sections s
  set head_version_id = inserted_version.id,
      inherited_from_section_id = null,
      updated_at = now()
  from inserted_version
  where s.id = inserted_version.section_id
  returning
    s.id as section_id,
    inserted_version.parent_version_id as old_version_id,
    inserted_version.id as new_version_id
)
insert into wiki_commit_changes
  (id, commit_id, section_id, old_version_id, new_version_id, change_type)
select
  gen_random_uuid(),
  commit_row.id,
  updated_section.section_id,
  updated_section.old_version_id,
  updated_section.new_version_id,
  'edit'
from updated_section
cross join commit_row;

update wiki_sections
set inherited_from_section_id = null,
    updated_at = now()
where inherited_from_section_id is not null;

update course_offerings
set inherited_from_offering_id = null,
    updated_at = now()
where inherited_from_offering_id is not null;
