CREATE OR REPLACE FUNCTION labflow_try_timestamptz(value TEXT) RETURNS TIMESTAMPTZ
LANGUAGE plpgsql STABLE AS '
BEGIN
    IF value !~ ''^[0-9]{4}-[0-9]{2}-[0-9]{2}([T ][0-9]{2}:[0-9]{2}.*)?$'' THEN
        RETURN NULL;
    END IF;
    RETURN value::TIMESTAMPTZ;
EXCEPTION WHEN invalid_datetime_format OR datetime_field_overflow OR invalid_text_representation THEN
    RETURN NULL;
END;
';
