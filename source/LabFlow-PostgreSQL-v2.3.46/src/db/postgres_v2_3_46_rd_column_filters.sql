-- Safe scalar extraction for configurable RD columns, including malformed legacy imports.
CREATE OR REPLACE FUNCTION labflow_rd_extra_text_or_empty(input TEXT, field TEXT, base_value TEXT DEFAULT NULL, rules TEXT DEFAULT '[]')
RETURNS TEXT LANGUAGE plpgsql IMMUTABLE AS
'DECLARE data jsonb; value jsonb; option_rules jsonb; selected TEXT; legacy_detail TEXT := ''''; detail TEXT; separator INTEGER; has_rule BOOLEAN;
 BEGIN
  BEGIN data := CAST(input AS jsonb);
   EXCEPTION WHEN data_exception THEN data := ''{}''::jsonb; END;
  value := data -> field;
  selected := base_value;
  IF selected IS NULL THEN
   IF jsonb_typeof(value) IN (''string'',''number'',''boolean'') THEN
    selected := value #>> ''{}'';
   ELSE selected := ''''; END IF;
  END IF;
  option_rules := CAST(rules AS jsonb);
  separator := strpos(selected, ''：'');
  IF separator > 1 AND EXISTS(SELECT 1 FROM jsonb_array_elements(option_rules) item WHERE item ->> ''trigger_value'' = btrim(substring(selected FROM 1 FOR separator - 1))) THEN
   legacy_detail := btrim(substring(selected FROM separator + 1));
   selected := btrim(substring(selected FROM 1 FOR separator - 1));
  END IF;
  has_rule := EXISTS(SELECT 1 FROM jsonb_array_elements(option_rules) item WHERE item ->> ''trigger_value'' = selected);
  detail := btrim(COALESCE(data ->> (field || ''__detail''), legacy_detail));
  IF has_rule AND detail <> '''' THEN RETURN selected || ''（'' || detail || ''）''; END IF;
  RETURN COALESCE(selected, '''');
  EXCEPTION WHEN data_exception THEN RETURN COALESCE(base_value, ''''); END;'
