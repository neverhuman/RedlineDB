CREATE TABLE parent(id INTEGER PRIMARY KEY, name TEXT);
CREATE TABLE child(id INTEGER, pid INTEGER, FOREIGN KEY (pid) REFERENCES parent(id));
INSERT INTO parent VALUES (1, 'p1'), (2, 'p2');
INSERT INTO child VALUES (10, 1), (11, 2);
SELECT child.id, parent.name FROM child JOIN parent ON child.pid = parent.id ORDER BY child.id
