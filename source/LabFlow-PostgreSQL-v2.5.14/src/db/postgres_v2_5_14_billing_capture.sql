CREATE OR REPLACE FUNCTION labflow_capture_billing_snapshot() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        IF NEW.unit_price_snapshot IS NOT NULL THEN
            RETURN NEW;
        END IF;
    ELSE
        IF NEW.method_id IS NOT DISTINCT FROM OLD.method_id OR NEW.method_id IS NULL THEN
            RETURN NEW;
        END IF;
    END IF;
    IF TG_TABLE_NAME = 'rd_work_records' THEN
        SELECT amount, COALESCE(multiplier, 1.0)
        INTO NEW.unit_price_snapshot, NEW.billing_multiplier_snapshot
        FROM methods WHERE id=NEW.method_id;
    ELSE
        SELECT amount INTO NEW.unit_price_snapshot FROM methods WHERE id=NEW.method_id;
    END IF;
    RETURN NEW;
END;
$$;
