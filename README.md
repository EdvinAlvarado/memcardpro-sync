# memcardpro-sync
Backups, Restores, and Synchronizes memcardpro memory cards to retroarch.

Currently works well with memcardpro cloud backups. local backups may bring errors if you have UNIROM installed in the memcardpro but it shoudn't fail to convert.

Goals
- [x] Convert MemCard-Pro mdc files to retroarch-ready srm files. 
- [ ] Convert from retroarch-ready srm files top MemCard-Pro formatted mdc files.
- [ ] Synchronize based on whether the mcd or the srm file was changed last.
- [ ] Add flag to optionally add language marker.
- [ ] Perform memcardpro backup by reading the memcardpro through FTP. Sadly memcardpro limitation of one transfer causes that a simple lftp mirror to fail.
