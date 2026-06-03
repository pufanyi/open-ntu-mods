create table email_login_codes (
  id uuid primary key,
  email text not null,
  code_hash text not null,
  expires_at timestamptz not null,
  consumed_at timestamptz null,
  attempts integer not null default 0,
  created_at timestamptz not null
);

create index email_login_codes_email_created_idx
  on email_login_codes (lower(email), created_at desc);

create index email_login_codes_expires_idx
  on email_login_codes (expires_at);
