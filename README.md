# memcardpro-sync
Backups, Restores, and Synchronizes memcardpro memory cards to retroarch.

Currently works well with memcardpro cloud backups. local backups may bring errors if you have UNIROM installed in the memcardpro but it shoudn't fail to convert.

Goals
- [x] Convert MemCard-Pro code-named mcd files to Swanstation/Duckstatioon title-named mcd files. 
- [ ] Convert from Swanstation/Duckstatioon title-named mcd  files top MemCard-Pro formatted mcd files.
- [ ] Implement synchronization.
- [ ] Add flag to optionally add language marker.
- [ ] Perform memcardpro backup by reading the memcardpro through FTP. Sadly memcardpro limitation of one transfer causes that a simple lftp mirror to fail.
