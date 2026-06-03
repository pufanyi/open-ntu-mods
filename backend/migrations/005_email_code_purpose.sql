alter table email_login_codes
  add column purpose text not null default 'login';

create index email_login_codes_email_purpose_created_idx
  on email_login_codes (lower(email), purpose, created_at desc);
