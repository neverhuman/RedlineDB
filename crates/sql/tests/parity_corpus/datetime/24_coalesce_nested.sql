-- datetime seed 24: nested coalesce
SELECT coalesce((SELECT NULL), coalesce(NULL, 'fallback'), 'unused');
