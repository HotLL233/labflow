CREATE OR REPLACE FUNCTION labflow_rd_timestamp_or_null(input TEXT)
RETURNS TIMESTAMP LANGUAGE plpgsql STABLE AS
'BEGIN
 IF input !~ ''^[0-9]{4}-[0-9]{2}-[0-9]{2}([T ][0-9]{2}:[0-9]{2}(:[0-9]{2}([.][0-9]+)?)?)?$'' THEN
   RETURN NULL;
 END IF;
 RETURN CAST(input AS timestamp);
 EXCEPTION WHEN data_exception THEN RETURN NULL; END;'
