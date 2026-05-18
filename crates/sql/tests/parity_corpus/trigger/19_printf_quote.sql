-- trigger seed 19: printf and quote
SELECT printf('%d-%s', 7, 'x'), quote(NULL), quote('text');
