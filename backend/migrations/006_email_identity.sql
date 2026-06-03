update users
set email = lower(email),
    provider_tenant_id = 'email',
    provider_user_id = lower(coalesce(provider_user_id, email)),
    updated_at = now()
where provider = 'email';

create unique index users_email_provider_identity_lower_idx
  on users (lower(provider_user_id))
  where provider = 'email'
    and provider_tenant_id = 'email'
    and provider_user_id is not null;

create unique index users_email_provider_email_lower_idx
  on users (lower(email))
  where provider = 'email';
