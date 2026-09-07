PRAGMA foreign_keys = ON;

UPDATE machines
SET name = 'Internetpraktikum',
    location = 'Internetpraktikum'
WHERE id = 'main';

DELETE FROM users
WHERE user_type = 'gitlab'
  AND gitlab_id BETWEEN 10000 AND 10009
  AND display_name IN (
      'Ada Lovelace',
      'Grace Hopper',
      'Katherine Johnson',
      'Edsger Dijkstra',
      'Barbara Liskov',
      'Donald Knuth',
      'Frances Allen',
      'Alan Turing',
      'Radia Perlman',
      'Ken Thompson'
  );
